use crate::constants::*;
use crate::{AttributeSet, InputEvent, InputEventKind, Key, KeyArray};
use bitvec::prelude::*;
use std::time::SystemTime;

/// A cached representation of device state at a certain time.
#[derive(Debug)]
pub struct DeviceState {
    /// The state corresponds to kernel state at this timestamp.
    pub(crate) timestamp: libc::timeval,
    /// Set = key pressed
    pub(crate) key_vals: Option<Box<KeyArray>>,
    pub(crate) abs_vals: Option<Box<[libc::input_absinfo; AbsoluteAxisType::COUNT]>>,
    /// Set = switch enabled (closed)
    pub(crate) switch_vals: Option<BitArr!(for SwitchType::COUNT, in u8)>,
    /// Set = LED lit
    pub(crate) led_vals: Option<BitArr!(for LedType::COUNT, in u8)>,
}

// manual Clone impl for clone_from optimization
impl Clone for DeviceState {
    fn clone(&self) -> Self {
        Self {
            timestamp: self.timestamp,
            key_vals: self.key_vals.clone(),
            abs_vals: self.abs_vals.clone(),
            switch_vals: self.switch_vals,
            led_vals: self.led_vals,
        }
    }
    fn clone_from(&mut self, other: &Self) {
        self.timestamp.clone_from(&other.timestamp);
        self.key_vals.clone_from(&other.key_vals);
        self.abs_vals.clone_from(&other.abs_vals);
        self.switch_vals.clone_from(&other.switch_vals);
        self.led_vals.clone_from(&other.led_vals);
    }
}

impl DeviceState {
    /// Returns the time when this snapshot was taken.
    pub fn timestamp(&self) -> SystemTime {
        crate::timeval_to_systime(&self.timestamp)
    }

    /// Returns the set of keys pressed when the snapshot was taken.
    ///
    /// Returns `None` if keys are not supported by this device.
    pub fn key_vals(&self) -> Option<AttributeSet<'_, Key>> {
        self.key_vals
            .as_deref()
            .map(|v| AttributeSet::new(BitSlice::from_slice(v).unwrap()))
    }

    /// Returns the set of absolute axis measurements when the snapshot was taken.
    ///
    /// Returns `None` if not supported by this device.
    pub fn abs_vals(&self) -> Option<&[libc::input_absinfo]> {
        self.abs_vals.as_deref().map(|v| &v[..])
    }

    /// Returns the set of switches triggered when the snapshot was taken.
    ///
    /// Returns `None` if switches are not supported by this device.
    pub fn switch_vals(&self) -> Option<AttributeSet<'_, SwitchType>> {
        self.switch_vals.as_deref().map(AttributeSet::new)
    }

    /// Returns the set of LEDs turned on when the snapshot was taken.
    ///
    /// Returns `None` if LEDs are not supported by this device.
    pub fn led_vals(&self) -> Option<AttributeSet<'_, LedType>> {
        self.led_vals.as_deref().map(AttributeSet::new)
    }

    #[inline]
    pub(crate) fn process_event(&mut self, ev: InputEvent) {
        match ev.kind() {
            InputEventKind::Key(code) => {
                let keys = self
                    .key_vals
                    .as_deref_mut()
                    .expect("got a key event despite not supporting keys");
                let keys = BitSlice::<Lsb0, _>::from_slice_mut(keys).unwrap();
                keys.set(code.code() as usize, ev.value() != 0);
            }
            InputEventKind::AbsAxis(axis) => {
                let axes = self
                    .abs_vals
                    .as_deref_mut()
                    .expect("got an abs event despite not supporting absolute axes");
                axes[axis.0 as usize].value = ev.value();
            }
            _ => {}
        }
    }
}