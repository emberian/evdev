use crate::constants::*;
use crate::raw_events::RawDevice;
use crate::{AttributeSet, InputEvent, InputEventKind, Key, KeyArray};
use bitvec::prelude::*;
use std::path::Path;
use std::time::SystemTime;
use std::{fmt, io};

pub struct Device {
    raw: RawDevice,
    state: DeviceState,
}

impl Device {
    /// Opens a device, given its system path.
    ///
    /// Paths are typically something like `/dev/input/event0`.
    #[inline(always)]
    pub fn open(path: impl AsRef<Path>) -> io::Result<Device> {
        Self::_open(path.as_ref())
    }

    fn _open(path: &Path) -> io::Result<Device> {
        let raw = RawDevice::open(path)?;

        let supports = raw.supported_events();

        let key_vals = if supports.contains(EventType::KEY) {
            Some(Box::new(crate::KEY_ARRAY_INIT))
        } else {
            None
        };
        let abs_vals = if supports.contains(EventType::ABSOLUTE) {
            #[rustfmt::skip]
            const ABSINFO_ZERO: libc::input_absinfo = libc::input_absinfo {
                value: 0, minimum: 0, maximum: 0, fuzz: 0, flat: 0, resolution: 0,
            };
            const ABS_VALS_INIT: [libc::input_absinfo; AbsoluteAxisType::COUNT] =
                [ABSINFO_ZERO; AbsoluteAxisType::COUNT];
            Some(Box::new(ABS_VALS_INIT))
        } else {
            None
        };
        let switch_vals = if supports.contains(EventType::SWITCH) {
            Some(BitArray::zeroed())
        } else {
            None
        };
        let led_vals = if supports.contains(EventType::LED) {
            Some(BitArray::zeroed())
        } else {
            None
        };

        let state = DeviceState {
            timestamp: libc::timeval {
                tv_sec: 0,
                tv_usec: 0,
            },
            key_vals,
            abs_vals,
            switch_vals,
            led_vals,
        };

        Ok(Device { raw, state })
    }

    /// Returns the device's name as read from the kernel.
    pub fn name(&self) -> Option<&str> {
        self.raw.name()
    }

    /// Returns the device's physical location, either as set by the caller or as read from the kernel.
    pub fn physical_path(&self) -> Option<&str> {
        self.raw.physical_path()
    }

    /// Returns the user-defined "unique name" of the device, if one has been set.
    pub fn unique_name(&self) -> Option<&str> {
        self.raw.unique_name()
    }

    /// Returns a struct containing bustype, vendor, product, and version identifiers
    pub fn input_id(&self) -> libc::input_id {
        self.raw.input_id()
    }

    /// Returns the set of supported "properties" for the device (see `INPUT_PROP_*` in kernel headers)
    pub fn properties(&self) -> AttributeSet<'_, PropType> {
        self.raw.properties()
    }

    /// Returns a tuple of the driver version containing major, minor, rev
    pub fn driver_version(&self) -> (u8, u8, u8) {
        self.raw.driver_version()
    }

    /// Returns a set of the event types supported by this device (Key, Switch, etc)
    ///
    /// If you're interested in the individual keys or switches supported, it's probably easier
    /// to just call the appropriate `supported_*` function instead.
    pub fn supported_events(&self) -> AttributeSet<'_, EventType> {
        self.raw.supported_events()
    }

    /// Returns the set of supported keys reported by the device.
    ///
    /// For keyboards, this is the set of all possible keycodes the keyboard may emit. Controllers,
    /// mice, and other peripherals may also report buttons as keys.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use evdev::{Device, Key};
    /// let device = Device::open("/dev/input/event0")?;
    ///
    /// // Does this device have an ENTER key?
    /// let supported = device.supported_keys().map_or(false, |keys| keys.contains(Key::KEY_ENTER));
    /// # Ok(())
    /// # }
    /// ```
    pub fn supported_keys(&self) -> Option<AttributeSet<'_, Key>> {
        self.raw.supported_keys()
    }

    /// Returns the set of supported "relative axes" reported by the device.
    ///
    /// Standard mice will generally report `REL_X` and `REL_Y` along with wheel if supported.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use evdev::{Device, RelativeAxisType};
    /// let device = Device::open("/dev/input/event0")?;
    ///
    /// // Does the device have a scroll wheel?
    /// let supported = device
    ///     .supported_relative_axes()
    ///     .map_or(false, |axes| axes.contains(RelativeAxisType::REL_WHEEL));
    /// # Ok(())
    /// # }
    /// ```
    pub fn supported_relative_axes(&self) -> Option<AttributeSet<'_, RelativeAxisType>> {
        self.raw.supported_relative_axes()
    }

    /// Returns the set of supported "absolute axes" reported by the device.
    ///
    /// These are most typically supported by joysticks and touchpads.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use evdev::{Device, AbsoluteAxisType};
    /// let device = Device::open("/dev/input/event0")?;
    ///
    /// // Does the device have an absolute X axis?
    /// let supported = device
    ///     .supported_absolute_axes()
    ///     .map_or(false, |axes| axes.contains(AbsoluteAxisType::ABS_X));
    /// # Ok(())
    /// # }
    /// ```
    pub fn supported_absolute_axes(&self) -> Option<AttributeSet<'_, AbsoluteAxisType>> {
        self.raw.supported_absolute_axes()
    }

    /// Returns the set of supported switches reported by the device.
    ///
    /// These are typically used for things like software switches on laptop lids (which the
    /// system reacts to by suspending or locking), or virtual switches to indicate whether a
    /// headphone jack is plugged in (used to disable external speakers).
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use evdev::{Device, SwitchType};
    /// let device = Device::open("/dev/input/event0")?;
    ///
    /// // Does the device report a laptop lid switch?
    /// let supported = device
    ///     .supported_switches()
    ///     .map_or(false, |axes| axes.contains(SwitchType::SW_LID));
    /// # Ok(())
    /// # }
    /// ```
    pub fn supported_switches(&self) -> Option<AttributeSet<'_, SwitchType>> {
        self.raw.supported_switches()
    }

    /// Returns a set of supported LEDs on the device.
    ///
    /// Most commonly these are state indicator lights for things like Scroll Lock, but they
    /// can also be found in cameras and other devices.
    pub fn supported_leds(&self) -> Option<AttributeSet<'_, LedType>> {
        self.raw.supported_leds()
    }

    /// Returns a set of supported "miscellaneous" capabilities.
    ///
    /// Aside from vendor-specific key scancodes, most of these are uncommon.
    pub fn misc_properties(&self) -> Option<AttributeSet<'_, MiscType>> {
        self.raw.misc_properties()
    }

    // pub fn supported_repeats(&self) -> Option<Repeat> {
    //     self.rep
    // }

    /// Returns the set of supported simple sounds supported by a device.
    ///
    /// You can use these to make really annoying beep sounds come from an internal self-test
    /// speaker, for instance.
    pub fn supported_sounds(&self) -> Option<AttributeSet<'_, SoundType>> {
        self.raw.supported_sounds()
    }

    /// Fetches and returns events from the kernel ring buffer, doing synchronization on SYN_DROPPED.
    ///
    /// By default this will block until events are available. Typically, users will want to call
    /// this in a tight loop within a thread.
    /// Will insert "fake" events.
    pub fn fetch_events(&mut self) -> io::Result<impl Iterator<Item = InputEvent> + '_> {
        self.raw.fill_events()?;
        let range = self._fetch_events()?;
        Ok(self.raw.event_buf.drain(range).map(InputEvent))
    }

    fn _fetch_events(&mut self) -> io::Result<std::ops::Range<usize>> {
        let mut block_start = 0;
        'outer: loop {
            let mut block_dropped = false;
            for (i, ev) in self.raw.event_buf.iter().enumerate().skip(block_start) {
                match InputEvent(*ev).kind() {
                    InputEventKind::Synchronization(Synchronization::SYN_DROPPED) => {
                        block_dropped = true;
                    }
                    InputEventKind::Synchronization(Synchronization::SYN_REPORT) => {
                        if block_dropped {
                            self.raw.event_buf.drain(block_start..=i);
                        }
                        block_start = i + 1;
                        continue 'outer;
                    }
                    _ => {}
                }
            }
            break;
        }
        Ok(0..block_start)
    }
}

impl fmt::Display for Device {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "{}:", self.name().unwrap_or("Unnamed device"))?;
        let (maj, min, pat) = self.driver_version();
        writeln!(f, "  Driver version: {}.{}.{}", maj, min, pat)?;
        if let Some(ref phys) = self.physical_path() {
            writeln!(f, "  Physical address: {:?}", phys)?;
        }
        if let Some(ref uniq) = self.unique_name() {
            writeln!(f, "  Unique name: {:?}", uniq)?;
        }

        let id = self.input_id();

        writeln!(f, "  Bus: {}", bus_name(id.bustype))?;
        writeln!(f, "  Vendor: {:#x}", id.vendor)?;
        writeln!(f, "  Product: {:#x}", id.product)?;
        writeln!(f, "  Version: {:#x}", id.version)?;
        writeln!(f, "  Properties: {:?}", self.properties())?;

        if let (Some(supported_keys), Some(key_vals)) =
            (self.supported_keys(), self.state.key_vals())
        {
            writeln!(f, "  Keys supported:")?;
            for key in supported_keys.iter() {
                let key_idx = key.code() as usize;
                writeln!(
                    f,
                    "    {:?} ({}index {})",
                    key,
                    if key_vals.contains(key) {
                        "pressed, "
                    } else {
                        ""
                    },
                    key_idx
                )?;
            }
        }

        if let Some(supported_relative) = self.supported_relative_axes() {
            writeln!(f, "  Relative Axes: {:?}", supported_relative)?;
        }

        if let (Some(supported_abs), Some(abs_vals)) =
            (self.supported_absolute_axes(), &self.state.abs_vals)
        {
            writeln!(f, "  Absolute Axes:")?;
            for abs in supported_abs.iter() {
                writeln!(
                    f,
                    "    {:?} ({:?}, index {})",
                    abs, abs_vals[abs.0 as usize], abs.0
                )?;
            }
        }

        if let Some(supported_misc) = self.misc_properties() {
            writeln!(f, "  Miscellaneous capabilities: {:?}", supported_misc)?;
        }

        if let (Some(supported_switch), Some(switch_vals)) =
            (self.supported_switches(), self.state.switch_vals())
        {
            writeln!(f, "  Switches:")?;
            for sw in supported_switch.iter() {
                writeln!(
                    f,
                    "    {:?} ({:?}, index {})",
                    sw,
                    switch_vals.contains(sw),
                    sw.0
                )?;
            }
        }

        if let (Some(supported_led), Some(led_vals)) =
            (self.supported_leds(), self.state.led_vals())
        {
            writeln!(f, "  LEDs:")?;
            for led in supported_led.iter() {
                writeln!(
                    f,
                    "    {:?} ({:?}, index {})",
                    led,
                    led_vals.contains(led),
                    led.0
                )?;
            }
        }

        if let Some(supported_snd) = self.supported_sounds() {
            write!(f, "  Sounds:")?;
            for snd in supported_snd.iter() {
                writeln!(f, "    {:?} (index {})", snd, snd.0)?;
            }
        }

        // if let Some(rep) = self.rep {
        //     writeln!(f, "  Repeats: {:?}", rep)?;
        // }

        let evs = self.supported_events();

        if evs.contains(EventType::FORCEFEEDBACK) {
            writeln!(f, "  Force Feedback supported")?;
        }

        if evs.contains(EventType::POWER) {
            writeln!(f, "  Power supported")?;
        }

        if evs.contains(EventType::FORCEFEEDBACKSTATUS) {
            writeln!(f, "  Force Feedback status supported")?;
        }

        Ok(())
    }
}

const fn bus_name(x: u16) -> &'static str {
    match x {
        0x1 => "PCI",
        0x2 => "ISA Plug 'n Play",
        0x3 => "USB",
        0x4 => "HIL",
        0x5 => "Bluetooth",
        0x6 => "Virtual",
        0x10 => "ISA",
        0x11 => "i8042",
        0x12 => "XTKBD",
        0x13 => "RS232",
        0x14 => "Gameport",
        0x15 => "Parallel Port",
        0x16 => "Amiga",
        0x17 => "ADB",
        0x18 => "I2C",
        0x19 => "Host",
        0x1A => "GSC",
        0x1B => "Atari",
        0x1C => "SPI",
        _ => "Unknown",
    }
}

/// A cached representation of device state at a certain time.
#[derive(Debug, Clone)]
pub struct DeviceState {
    /// The state corresponds to kernel state at this timestamp.
    timestamp: libc::timeval,
    /// Set = key pressed
    key_vals: Option<Box<KeyArray>>,
    abs_vals: Option<Box<[libc::input_absinfo; AbsoluteAxisType::COUNT]>>,
    /// Set = switch enabled (closed)
    switch_vals: Option<BitArr!(for SwitchType::COUNT, in u8)>,
    /// Set = LED lit
    led_vals: Option<BitArr!(for LedType::COUNT, in u8)>,
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
}
