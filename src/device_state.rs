use crate::compat::input_absinfo;
use crate::{constants::*, raw_stream::RawDevice};
use crate::{AttributeSet, AttributeSetRef, InputEvent, InputEventKind, Key};
use std::time::SystemTime;

/// A **cached** representation of device state at a certain time.
#[derive(Debug)]
pub struct DeviceState {
    /// The state corresponds to kernel state at this timestamp.
    pub(crate) timestamp: SystemTime,
    /// Set = key pressed
    pub(crate) key_vals: Option<AttributeSet<Key>>,
    pub(crate) abs_vals: Option<Box<[input_absinfo; AbsoluteAxisType::COUNT]>>,
    /// Set = switch enabled (closed)
    pub(crate) switch_vals: Option<AttributeSet<SwitchType>>,
    /// Set = LED lit
    pub(crate) led_vals: Option<AttributeSet<LedType>>,
}

// manual Clone impl for clone_from optimization
impl Clone for DeviceState {
    fn clone(&self) -> Self {
        Self {
            timestamp: self.timestamp,
            key_vals: self.key_vals.clone(),
            abs_vals: self.abs_vals.clone(),
            switch_vals: self.switch_vals.clone(),
            led_vals: self.led_vals.clone(),
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
    /// Create an empty `DeviceState`. The `{abs,key,etc}_vals` for the returned state will return
    /// `Some` if `supported_events()` contains that `EventType`.
    pub(crate) fn new(device: &RawDevice) -> Self {
        let supports = device.supported_events();

        let key_vals = if supports.contains(EventType::KEY) {
            Some(AttributeSet::new())
        } else {
            None
        };
        let abs_vals = if supports.contains(EventType::ABSOLUTE) {
            Some(Box::new(crate::raw_stream::ABS_VALS_INIT))
        } else {
            None
        };
        let switch_vals = if supports.contains(EventType::SWITCH) {
            Some(AttributeSet::new())
        } else {
            None
        };
        let led_vals = if supports.contains(EventType::LED) {
            Some(AttributeSet::new())
        } else {
            None
        };

        DeviceState {
            timestamp: std::time::UNIX_EPOCH,
            key_vals,
            abs_vals,
            switch_vals,
            led_vals,
        }
    }
    /// Returns the time when this snapshot was taken.
    pub fn timestamp(&self) -> SystemTime {
        self.timestamp
    }

    /// Returns the set of keys pressed when the snapshot was taken.
    ///
    /// Returns `None` if keys are not supported by this device.
    pub fn key_vals(&self) -> Option<&AttributeSetRef<Key>> {
        self.key_vals.as_deref()
    }

    /// Returns the set of absolute axis measurements when the snapshot was taken.
    ///
    /// Returns `None` if not supported by this device.
    pub fn abs_vals(&self) -> Option<&[input_absinfo]> {
        self.abs_vals.as_deref().map(|v| &v[..])
    }

    /// Returns the set of switches triggered when the snapshot was taken.
    ///
    /// Returns `None` if switches are not supported by this device.
    pub fn switch_vals(&self) -> Option<&AttributeSetRef<SwitchType>> {
        self.switch_vals.as_deref()
    }

    /// Returns the set of LEDs turned on when the snapshot was taken.
    ///
    /// Returns `None` if LEDs are not supported by this device.
    pub fn led_vals(&self) -> Option<&AttributeSetRef<LedType>> {
        self.led_vals.as_deref()
    }

    #[inline]
    pub(crate) fn process_event(&mut self, ev: InputEvent) {
        match ev.kind() {
            InputEventKind::Key(code) => {
                let keys = self
                    .key_vals
                    .as_deref_mut()
                    .expect("got a key event despite not supporting keys");
                keys.set(code, ev.value() != 0);
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
