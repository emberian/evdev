//! Linux event device handling.
//!
//! The Linux kernel's "evdev" subsystem exposes input devices to userspace in a generic,
//! consistent way. I'll try to explain the device model as completely as possible. The upstream
//! kernel documentation is split across two files:
//!
//! - https://www.kernel.org/doc/Documentation/input/event-codes.txt
//! - https://www.kernel.org/doc/Documentation/input/multi-touch-protocol.txt
//!
//! Devices emit events, represented by the [`InputEvent`] type. Each device supports a few different
//! kinds of events, specified by the [`EventType`] struct and the [`Device::supported_events()`]
//! method. Most event types also have a "subtype", e.g. a `KEY` event with a `KEY_ENTER` code. This
//! type+subtype combo is represented by [`InputEventKind`]/[`InputEvent::kind()`]. The individual
//! subtypes of a type that a device supports can be retrieved through the `Device::supported_*()`
//! methods, e.g. [`Device::supported_keys()`]:
//!
//! ```no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use evdev::{Device, Key};
//! let device = Device::open("/dev/input/event0")?;
//! // check if the device has an ENTER key
//! if device.supported_keys().map_or(false, |keys| keys.contains(Key::KEY_ENTER)) {
//!     println!("are you prepared to ENTER the world of evdev?");
//! } else {
//!     println!(":(");
//! }
//! # Ok(())
//! # }
//! ```
//!
//! This state can be queried. For example, the [`DeviceState::led_vals`] method will tell you which
//! LEDs are currently lit on the device. This state is not automatically synchronized with the
//! kernel. However, as the application reads events, this state will be updated if the event is
//! newer than the state timestamp (maintained internally).  Additionally, you can call
//! [`Device::sync_state`] to explicitly synchronize with the kernel state.
//!
//! As the state changes, the kernel will write events into a ring buffer. The application can read
//! from this ring buffer, thus retrieving events. However, if the ring buffer becomes full, the
//! kernel will *drop* every event in the ring buffer and leave an event telling userspace that it
//! did so. At this point, if the application were using the events it received to update its
//! internal idea of what state the hardware device is in, it will be wrong: it is missing some
//! events. This library tries to ease that pain, but it is best-effort. Events can never be
//! recovered once lost. For example, if a switch is toggled twice, there will be two switch events
//! in the buffer. However if the kernel needs to drop events, when the device goes to synchronize
//! state with the kernel, only one (or zero, if the switch is in the same state as it was before
//! the sync) switch events will be emulated.
//!
//! It is recommended that you dedicate a thread to processing input events, or use epoll or an
//! async runtime with the fd returned by `<Device as AsRawFd>::as_raw_fd` to process events when
//! they are ready.

#![cfg(any(unix, target_os = "android"))]
#![allow(non_camel_case_types)]

// has to be first for its macro
#[macro_use]
mod attribute_set;

mod constants;
pub mod raw;
mod scancodes;

use bitvec::prelude::*;
use std::fmt;
use std::fs::File;
use std::fs::OpenOptions;
use std::mem;
use std::os::unix::{
    fs::OpenOptionsExt,
    io::{AsRawFd, RawFd},
};
use std::path::Path;
use std::time::{Duration, SystemTime};
use std::{ffi::CString, mem::MaybeUninit};

// pub use crate::constants::FFEffect::*;
pub use crate::attribute_set::AttributeSet;
pub use crate::constants::*;
pub use crate::scancodes::*;
pub use crate::Synchronization::*;

fn ioctl_get_cstring(
    f: unsafe fn(RawFd, &mut [u8]) -> nix::Result<libc::c_int>,
    fd: RawFd,
) -> Option<CString> {
    const CAPACITY: usize = 256;
    let mut buf = vec![0; CAPACITY];
    match unsafe { f(fd, buf.as_mut_slice()) } {
        Ok(len) if len as usize > CAPACITY => {
            panic!("ioctl_get_cstring call overran the provided buffer!");
        }
        Ok(len) if len > 0 => {
            // Our ioctl string functions apparently return the number of bytes written, including
            // trailing \0.
            buf.truncate(len as usize);
            assert_eq!(buf.pop().unwrap(), 0);
            CString::new(buf).ok()
        }
        Ok(_) => {
            // if len < 0 => Explicit errno
            None
        }
        Err(_) => None,
    }
}

#[repr(u16)]
#[derive(Copy, Clone, Debug)]
pub enum Synchronization {
    /// Terminates a packet of events from the device.
    SYN_REPORT = 0,
    /// Appears to be unused.
    SYN_CONFIG = 1,
    /// "Used to synchronize and separate touch events"
    SYN_MT_REPORT = 2,
    /// Ring buffer filled, events were dropped.
    SYN_DROPPED = 3,
}

const fn bit_elts<T>(bits: usize) -> usize {
    let width = mem::size_of::<T>() * 8;
    bits / width + (bits % width != 0) as usize
}
// TODO: this is a replacement for BitArr!(for Key::COUNT, in u8), since const generics aren't stable
// and the BitView impls for arrays only goes up to 64
type KeyArray = [u8; bit_elts::<u8>(Key::COUNT)];

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
    pub fn key_vals(&self) -> Option<AttributeSet<'_, Key>> {
        self.key_vals
            .as_deref()
            .map(|v| AttributeSet::new(BitSlice::from_slice(v).unwrap()))
    }

    pub fn timestamp(&self) -> SystemTime {
        timeval_to_systime(&self.timestamp)
    }

    pub fn abs_vals(&self) -> Option<&[libc::input_absinfo]> {
        self.abs_vals.as_deref().map(|v| &v[..])
    }

    pub fn switch_vals(&self) -> Option<AttributeSet<'_, SwitchType>> {
        self.switch_vals.as_deref().map(AttributeSet::new)
    }

    pub fn led_vals(&self) -> Option<AttributeSet<'_, LedType>> {
        self.led_vals.as_deref().map(AttributeSet::new)
    }
}

impl Default for DeviceState {
    fn default() -> Self {
        DeviceState {
            timestamp: libc::timeval {
                tv_sec: 0,
                tv_usec: 0,
            },
            key_vals: None,
            abs_vals: None,
            switch_vals: None,
            led_vals: None,
        }
    }
}

/// Publicly visible errors which can be returned from evdev
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("libc/system error: {0}")]
    NixError(#[from] nix::Error),
    #[error("standard i/o error: {0}")]
    StdIoError(#[from] std::io::Error),
}

#[derive(Debug)]
pub struct Device {
    file: File,
    ty: BitArr!(for EventType::COUNT, in u8),
    name: Option<String>,
    phys: Option<String>,
    uniq: Option<String>,
    id: libc::input_id,
    props: BitArr!(for PropType::COUNT, in u8),
    driver_version: (u8, u8, u8),
    supported_keys: Option<Box<KeyArray>>,
    supported_relative: Option<BitArr!(for RelativeAxisType::COUNT, in u8)>,
    supported_absolute: Option<BitArr!(for AbsoluteAxisType::COUNT, in u8)>,
    supported_switch: Option<BitArr!(for SwitchType::COUNT, in u8)>,
    supported_led: Option<BitArr!(for LedType::COUNT, in u8)>,
    supported_misc: Option<BitArr!(for MiscType::COUNT, in u8)>,
    // ff: Option<Box<BitArr!(for _, in u8)>>,
    // ff_stat: Option<FFStatus>,
    // rep: Option<Repeat>,
    supported_snd: Option<BitArr!(for SoundType::COUNT, in u8)>,
    pending_events: Vec<libc::input_event>,
    // pending_events[last_seen..] is the events that have occurred since the last sync.
    last_seen: usize,
    state: DeviceState,
}

impl AsRawFd for Device {
    fn as_raw_fd(&self) -> RawFd {
        self.file.as_raw_fd()
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

impl fmt::Display for Device {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "{}:", self.name.as_deref().unwrap_or("Unnamed device"))?;
        writeln!(
            f,
            "  Driver version: {}.{}.{}",
            self.driver_version.0, self.driver_version.1, self.driver_version.2
        )?;
        if let Some(ref phys) = self.phys {
            writeln!(f, "  Physical address: {:?}", phys)?;
        }
        if let Some(ref uniq) = self.uniq {
            writeln!(f, "  Unique name: {:?}", uniq)?;
        }

        writeln!(f, "  Bus: {}", bus_name(self.id.bustype))?;
        writeln!(f, "  Vendor: {:#x}", self.id.vendor)?;
        writeln!(f, "  Product: {:#x}", self.id.product)?;
        writeln!(f, "  Version: {:#x}", self.id.version)?;
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
            (self.supported_absolute, &self.state.abs_vals)
        {
            writeln!(f, "  Absolute Axes:")?;
            for idx in supported_abs.iter_ones() {
                let abs = AbsoluteAxisType(idx as u16);
                writeln!(f, "    {:?} ({:?}, index {})", abs, abs_vals[idx], idx)?;
            }
        }

        if let Some(supported_misc) = self.misc_properties() {
            writeln!(f, "  Miscellaneous capabilities: {:?}", supported_misc)?;
        }

        if let (Some(supported_switch), Some(switch_vals)) =
            (self.supported_switch, &self.state.switch_vals)
        {
            writeln!(f, "  Switches:")?;
            for idx in supported_switch.iter_ones() {
                let sw = SwitchType(idx as u16);
                writeln!(f, "    {:?} ({:?}, index {})", sw, switch_vals[idx], idx)?;
            }
        }

        if let (Some(supported_led), Some(led_vals)) = (self.supported_led, &self.state.led_vals) {
            writeln!(f, "  LEDs:")?;
            for idx in supported_led.iter_ones() {
                let led = LedType(idx as u16);
                writeln!(f, "    {:?} ({:?}, index {})", led, led_vals[idx], idx)?;
            }
        }

        if let Some(supported_snd) = self.supported_snd {
            write!(f, "  Sounds:")?;
            for idx in supported_snd.iter_ones() {
                let snd = SoundType(idx as u16);
                writeln!(f, "    {:?} (index {})", snd, idx)?;
            }
        }

        // if let Some(rep) = self.rep {
        //     writeln!(f, "  Repeats: {:?}", rep)?;
        // }

        if self.ty[EventType::FORCEFEEDBACK.0 as usize] {
            writeln!(f, "  Force Feedback supported")?;
        }

        if self.ty[EventType::POWER.0 as usize] {
            writeln!(f, "  Power supported")?;
        }

        if self.ty[EventType::FORCEFEEDBACKSTATUS.0 as usize] {
            writeln!(f, "  Force Feedback status supported")?;
        }

        Ok(())
    }
}

impl Device {
    pub fn supported_events(&self) -> AttributeSet<'_, EventType> {
        AttributeSet::new(&self.ty)
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn physical_path(&self) -> Option<&str> {
        self.phys.as_deref()
    }

    pub fn unique_name(&self) -> Option<&str> {
        self.uniq.as_deref()
    }

    pub fn input_id(&self) -> libc::input_id {
        self.id
    }

    pub fn properties(&self) -> AttributeSet<'_, PropType> {
        AttributeSet::new(&self.props)
    }

    pub fn driver_version(&self) -> (u8, u8, u8) {
        self.driver_version
    }

    pub fn supported_keys(&self) -> Option<AttributeSet<'_, Key>> {
        self.supported_keys
            .as_deref()
            .map(|v| AttributeSet::new(BitSlice::from_slice(v).unwrap()))
    }

    pub fn supported_relative_axes(&self) -> Option<AttributeSet<'_, RelativeAxisType>> {
        self.supported_relative.as_deref().map(AttributeSet::new)
    }

    pub fn supported_absolute_axes(&self) -> Option<AttributeSet<'_, AbsoluteAxisType>> {
        self.supported_absolute.as_deref().map(AttributeSet::new)
    }

    pub fn supported_switches(&self) -> Option<AttributeSet<'_, SwitchType>> {
        self.supported_switch.as_deref().map(AttributeSet::new)
    }

    pub fn supported_leds(&self) -> Option<AttributeSet<'_, LedType>> {
        self.supported_led.as_deref().map(AttributeSet::new)
    }

    pub fn misc_properties(&self) -> Option<AttributeSet<'_, MiscType>> {
        self.supported_misc.as_deref().map(AttributeSet::new)
    }

    // pub fn supported_repeats(&self) -> Option<Repeat> {
    //     self.rep
    // }

    pub fn supported_sounds(&self) -> Option<AttributeSet<'_, SoundType>> {
        self.supported_snd.as_deref().map(AttributeSet::new)
    }

    pub fn state(&self) -> &DeviceState {
        &self.state
    }

    #[inline(always)]
    pub fn open(path: impl AsRef<Path>) -> Result<Device, Error> {
        Self::_open(path.as_ref())
    }

    fn _open(path: &Path) -> Result<Device, Error> {
        let mut options = OpenOptions::new();

        // Try to load read/write, then fall back to read-only.
        let file = options
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(path)
            .or_else(|_| options.write(false).open(path))?;

        let ty = {
            let mut ty = BitArray::zeroed();
            unsafe { raw::eviocgbit_type(file.as_raw_fd(), ty.as_mut_raw_slice())? };
            ty
        };

        let name = ioctl_get_cstring(raw::eviocgname, file.as_raw_fd())
            .map(|s| s.to_string_lossy().into_owned());
        let phys = ioctl_get_cstring(raw::eviocgphys, file.as_raw_fd())
            .map(|s| s.to_string_lossy().into_owned());
        let uniq = ioctl_get_cstring(raw::eviocguniq, file.as_raw_fd())
            .map(|s| s.to_string_lossy().into_owned());

        let id = unsafe {
            let mut id = MaybeUninit::uninit();
            raw::eviocgid(file.as_raw_fd(), id.as_mut_ptr())?;
            id.assume_init()
        };
        let mut driver_version: i32 = 0;
        unsafe {
            raw::eviocgversion(file.as_raw_fd(), &mut driver_version)?;
        }
        let driver_version = (
            ((driver_version >> 16) & 0xff) as u8,
            ((driver_version >> 8) & 0xff) as u8,
            (driver_version & 0xff) as u8,
        );

        let props = {
            let mut props = BitArray::zeroed();
            unsafe { raw::eviocgprop(file.as_raw_fd(), props.as_mut_raw_slice())? };
            props
        }; // FIXME: handle old kernel

        let mut state = DeviceState::default();

        let supported_keys = if ty[EventType::KEY.0 as usize] {
            const KEY_ARR_INIT: KeyArray = [0; bit_elts::<u8>(Key::COUNT)];

            state.key_vals = Some(Box::new(KEY_ARR_INIT));

            let mut supported_keys = Box::new(KEY_ARR_INIT);
            let key_slice = &mut supported_keys[..];
            unsafe { raw::eviocgbit_key(file.as_raw_fd(), key_slice)? };

            Some(supported_keys)
        } else {
            None
        };

        let supported_relative = if ty[EventType::RELATIVE.0 as usize] {
            let mut rel = BitArray::zeroed();
            unsafe { raw::eviocgbit_relative(file.as_raw_fd(), rel.as_mut_raw_slice())? };
            Some(rel)
        } else {
            None
        };

        let supported_absolute = if ty[EventType::ABSOLUTE.0 as usize] {
            #[rustfmt::skip]
            const ABSINFO_ZERO: libc::input_absinfo = libc::input_absinfo {
                value: 0, minimum: 0, maximum: 0, fuzz: 0, flat: 0, resolution: 0,
            };
            const ABS_VALS_INIT: [libc::input_absinfo; AbsoluteAxisType::COUNT] =
                [ABSINFO_ZERO; AbsoluteAxisType::COUNT];
            state.abs_vals = Some(Box::new(ABS_VALS_INIT));
            let mut abs = BitArray::zeroed();
            unsafe { raw::eviocgbit_absolute(file.as_raw_fd(), abs.as_mut_raw_slice())? };
            Some(abs)
        } else {
            None
        };

        let supported_switch = if ty[EventType::SWITCH.0 as usize] {
            state.switch_vals = Some(BitArray::zeroed());
            let mut switch = BitArray::zeroed();
            unsafe { raw::eviocgbit_switch(file.as_raw_fd(), switch.as_mut_raw_slice())? };
            Some(switch)
        } else {
            None
        };

        let supported_led = if ty[EventType::LED.0 as usize] {
            state.led_vals = Some(BitArray::zeroed());
            let mut led = BitArray::zeroed();
            unsafe { raw::eviocgbit_led(file.as_raw_fd(), led.as_mut_raw_slice())? };
            Some(led)
        } else {
            None
        };

        let supported_misc = if ty[EventType::MISC.0 as usize] {
            let mut misc = BitArray::zeroed();
            unsafe { raw::eviocgbit_misc(file.as_raw_fd(), misc.as_mut_raw_slice())? };
            Some(misc)
        } else {
            None
        };

        //unsafe { raw::eviocgbit(file.as_raw_fd(), ffs(FORCEFEEDBACK.bits()), 0x7f, bits_as_u8_slice)?; }

        let supported_snd = if ty[EventType::SOUND.0 as usize] {
            let mut snd = BitArray::zeroed();
            unsafe { raw::eviocgbit_sound(file.as_raw_fd(), snd.as_mut_raw_slice())? };
            Some(snd)
        } else {
            None
        };

        let mut dev = Device {
            file,
            ty,
            name,
            phys,
            uniq,
            id,
            props,
            driver_version,
            supported_keys,
            supported_relative,
            supported_absolute,
            supported_switch,
            supported_led,
            supported_misc,
            supported_snd,
            pending_events: Vec::with_capacity(64),
            last_seen: 0,
            state,
        };

        dev.sync_state()?;

        Ok(dev)
    }

    /// Synchronize the `Device` state with the kernel device state.
    ///
    /// If there is an error at any point, the state will not be synchronized completely.
    pub fn sync_state(&mut self) -> Result<(), Error> {
        let fd = self.as_raw_fd();
        if let Some(key_vals) = &mut self.state.key_vals {
            unsafe { raw::eviocgkey(fd, &mut key_vals[..])? };
        }

        if let (Some(supported_abs), Some(abs_vals)) =
            (self.supported_absolute, &mut self.state.abs_vals)
        {
            for idx in supported_abs.iter_ones() {
                // ignore multitouch, we'll handle that later.
                //
                // handling later removed. not sure what the intention of "handling that later" was
                // the abs data seems to be fine (tested ABS_MT_POSITION_X/Y)
                unsafe {
                    raw::eviocgabs(fd, idx as u32, &mut abs_vals[idx])?;
                }
            }
        }

        if let Some(switch_vals) = &mut self.state.switch_vals {
            unsafe { raw::eviocgsw(fd, switch_vals.as_mut_raw_slice())? };
        }

        if let Some(led_vals) = &mut self.state.led_vals {
            unsafe { raw::eviocgled(fd, led_vals.as_mut_raw_slice())? };
        }

        Ok(())
    }

    /// Do SYN_DROPPED synchronization, and compensate for missing events by inserting events into
    /// the stream which, when applied to any state being kept outside of this `Device`, will
    /// synchronize it with the kernel state.
    fn compensate_dropped(&mut self) -> Result<(), Error> {
        let mut drop_from = None;
        for (idx, event) in self.pending_events[self.last_seen..].iter().enumerate() {
            if event.type_ == SYN_DROPPED as u16 {
                drop_from = Some(idx);
                break;
            }
        }
        // FIXME: see if we can *not* drop EV_REL events. EV_REL doesn't have any state, so
        // dropping its events isn't really helping much.
        if let Some(idx) = drop_from {
            // look for the nearest SYN_REPORT before the SYN_DROPPED, remove everything after it.
            let mut prev_report = 0; // (if there's no previous SYN_REPORT, then the entire vector is bogus)
            for (idx, event) in self.pending_events[..idx].iter().enumerate().rev() {
                if event.type_ == SYN_REPORT as u16 {
                    prev_report = idx;
                    break;
                }
            }
            self.pending_events.truncate(prev_report);
        } else {
            return Ok(());
        }

        // Alright, pending_events is in a sane state. Now, let's sync the local state. We will
        // create a phony packet that contains deltas from the previous device state to the current
        // device state.
        let old_state = self.state.clone();
        self.sync_state()?;

        let time = systime_to_timeval(&SystemTime::now());

        if let (Some(supported_keys), Some(key_vals)) =
            (&self.supported_keys, self.state.key_vals())
        {
            let supported_keys =
                AttributeSet::new(BitSlice::from_slice(&supported_keys[..]).unwrap());
            let old_vals = old_state.key_vals();
            for key in supported_keys.iter() {
                if old_vals.map(|v| v.contains(key)) != Some(key_vals.contains(key)) {
                    self.pending_events.push(libc::input_event {
                        time,
                        type_: EventType::KEY.0 as _,
                        code: key.code() as u16,
                        value: if key_vals.contains(key) { 1 } else { 0 },
                    });
                }
            }
        }

        if let (Some(supported_abs), Some(abs_vals)) =
            (self.supported_absolute, &self.state.abs_vals)
        {
            for idx in supported_abs.iter_ones() {
                if old_state.abs_vals.as_ref().map(|v| v[idx]) != Some(abs_vals[idx]) {
                    self.pending_events.push(libc::input_event {
                        time,
                        type_: EventType::ABSOLUTE.0 as _,
                        code: idx as u16,
                        value: abs_vals[idx].value,
                    });
                }
            }
        }

        if let (Some(supported_switch), Some(switch_vals)) =
            (self.supported_switch, &self.state.switch_vals)
        {
            for idx in supported_switch.iter_ones() {
                if old_state.switch_vals.as_ref().map(|v| v[idx]) != Some(switch_vals[idx]) {
                    self.pending_events.push(libc::input_event {
                        time,
                        type_: EventType::SWITCH.0 as _,
                        code: idx as u16,
                        value: if switch_vals[idx] { 1 } else { 0 },
                    });
                }
            }
        }

        if let (Some(supported_led), Some(led_vals)) = (self.supported_led, &self.state.led_vals) {
            for idx in supported_led.iter_ones() {
                if old_state.led_vals.as_ref().map(|v| v[idx]) != Some(led_vals[idx]) {
                    self.pending_events.push(libc::input_event {
                        time,
                        type_: EventType::LED.0 as _,
                        code: idx as u16,
                        value: if led_vals[idx] { 1 } else { 0 },
                    });
                }
            }
        }

        self.pending_events.push(libc::input_event {
            time,
            type_: EventType::SYNCHRONIZATION.0 as _,
            code: SYN_REPORT as u16,
            value: 0,
        });
        Ok(())
    }

    fn fill_events(&mut self) -> Result<(), Error> {
        let fd = self.as_raw_fd();
        let buf = &mut self.pending_events;
        loop {
            buf.reserve(20);
            // TODO: use Vec::spare_capacity_mut or Vec::split_at_spare_mut when they stabilize
            let spare_capacity = vec_spare_capacity_mut(buf);
            let (_, uninit_buf, _) =
                unsafe { spare_capacity.align_to_mut::<mem::MaybeUninit<u8>>() };

            // use libc::read instead of nix::unistd::read b/c we need to pass an uninitialized buf
            let res = unsafe { libc::read(fd, uninit_buf.as_mut_ptr() as _, uninit_buf.len()) };
            match nix::errno::Errno::result(res) {
                Ok(bytes_read) => unsafe {
                    let pre_len = buf.len();
                    buf.set_len(
                        pre_len + (bytes_read as usize / mem::size_of::<libc::input_event>()),
                    );
                },
                Err(e) => {
                    if e == nix::Error::Sys(::nix::errno::Errno::EAGAIN) {
                        break;
                    } else {
                        return Err(e.into());
                    }
                }
            }
        }
        Ok(())
    }

    /// Exposes the raw evdev events without doing synchronization on SYN_DROPPED.
    pub fn events_no_sync(&mut self) -> Result<RawEvents, Error> {
        self.fill_events()?;
        Ok(RawEvents::new(self))
    }

    /// Exposes the raw evdev events, doing synchronization on SYN_DROPPED.
    ///
    /// Will insert "fake" events
    pub fn events(&mut self) -> Result<RawEvents, Error> {
        self.fill_events()?;
        self.compensate_dropped()?;

        Ok(RawEvents::new(self))
    }

    pub fn wait_ready(&self) -> nix::Result<()> {
        use nix::poll;
        let mut pfd = poll::PollFd::new(self.as_raw_fd(), poll::PollFlags::POLLIN);
        poll::poll(std::slice::from_mut(&mut pfd), -1)?;
        Ok(())
    }
}

pub struct Events<'a>(&'a mut Device);

pub struct RawEvents<'a>(&'a mut Device);

impl<'a> RawEvents<'a> {
    fn new(dev: &'a mut Device) -> RawEvents<'a> {
        dev.pending_events.reverse();
        RawEvents(dev)
    }
}

impl<'a> Drop for RawEvents<'a> {
    fn drop(&mut self) {
        self.0.pending_events.reverse();
        self.0.last_seen = self.0.pending_events.len();
    }
}

impl<'a> Iterator for RawEvents<'a> {
    type Item = InputEvent;

    #[inline(always)]
    fn next(&mut self) -> Option<InputEvent> {
        self.0.pending_events.pop().map(InputEvent)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum InputEventKind {
    Synchronization,
    Key(Key),
    RelAxis(RelativeAxisType),
    AbsAxis(AbsoluteAxisType),
    Misc(MiscType),
    Switch(SwitchType),
    Led(LedType),
    Sound(SoundType),
    Other,
}

#[repr(transparent)]
pub struct InputEvent(libc::input_event);

impl InputEvent {
    #[inline]
    pub fn timestamp(&self) -> SystemTime {
        timeval_to_systime(&self.0.time)
    }

    #[inline]
    pub fn event_type(&self) -> EventType {
        EventType(self.0.type_)
    }

    #[inline]
    pub fn code(&self) -> u16 {
        self.0.code
    }

    /// A convenience function to return `self.code()` wrapped in a certain newtype determined by
    /// the type of this event.
    #[inline]
    pub fn kind(&self) -> InputEventKind {
        let code = self.code();
        match self.event_type() {
            EventType::SYNCHRONIZATION => InputEventKind::Synchronization,
            EventType::KEY => InputEventKind::Key(Key::new(code)),
            EventType::RELATIVE => InputEventKind::RelAxis(RelativeAxisType(code)),
            EventType::ABSOLUTE => InputEventKind::AbsAxis(AbsoluteAxisType(code)),
            EventType::MISC => InputEventKind::Misc(MiscType(code)),
            EventType::SWITCH => InputEventKind::Switch(SwitchType(code)),
            EventType::LED => InputEventKind::Led(LedType(code)),
            EventType::SOUND => InputEventKind::Sound(SoundType(code)),
            _ => InputEventKind::Other,
        }
    }

    #[inline]
    pub fn value(&self) -> i32 {
        self.0.value
    }

    pub fn from_raw(raw: libc::input_event) -> Self {
        Self(raw)
    }

    pub fn as_raw(&self) -> &libc::input_event {
        &self.0
    }
}

impl fmt::Debug for InputEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut debug = f.debug_struct("InputEvent");
        debug.field("time", &self.timestamp());
        let kind = self.kind();
        if let InputEventKind::Other = kind {
            debug
                .field("type", &self.event_type())
                .field("code", &self.code());
        } else {
            debug.field("kind", &kind);
        }
        debug.field("value", &self.value()).finish()
    }
}

/// Crawls `/dev/input` for evdev devices.
///
/// Will not bubble up any errors in opening devices or traversing the directory. Instead returns
/// an empty iterator or omits the devices that could not be opened.
pub fn enumerate() -> EnumerateDevices {
    EnumerateDevices {
        readdir: std::fs::read_dir("/dev/input").ok(),
    }
}

pub struct EnumerateDevices {
    readdir: Option<std::fs::ReadDir>,
}
impl Iterator for EnumerateDevices {
    type Item = Device;
    fn next(&mut self) -> Option<Device> {
        let readdir = self.readdir.as_mut()?;
        loop {
            if let Ok(entry) = readdir.next()? {
                if let Ok(dev) = Device::open(entry.path()) {
                    return Some(dev);
                }
            }
        }
    }
}

/// A safe Rust version of clock_gettime against CLOCK_REALTIME
fn systime_to_timeval(time: &SystemTime) -> libc::timeval {
    let (sign, dur) = match time.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(dur) => (1, dur),
        Err(e) => (-1, e.duration()),
    };

    libc::timeval {
        tv_sec: dur.as_secs() as libc::time_t * sign,
        tv_usec: dur.subsec_micros() as libc::suseconds_t,
    }
}

fn timeval_to_systime(tv: &libc::timeval) -> SystemTime {
    let dur = Duration::new(tv.tv_sec.abs() as u64, tv.tv_usec as u32 * 1000);
    if tv.tv_sec >= 0 {
        SystemTime::UNIX_EPOCH + dur
    } else {
        SystemTime::UNIX_EPOCH - dur
    }
}

/// A copy of the unstable Vec::spare_capacity_mut
#[inline]
fn vec_spare_capacity_mut<T>(v: &mut Vec<T>) -> &mut [mem::MaybeUninit<T>] {
    let (len, cap) = (v.len(), v.capacity());
    unsafe {
        std::slice::from_raw_parts_mut(
            v.as_mut_ptr().add(len) as *mut mem::MaybeUninit<T>,
            cap - len,
        )
    }
}

#[cfg(test)]
mod test {
    use std::mem::MaybeUninit;

    #[test]
    fn align_to_mut_is_sane() {
        // We assume align_to_mut -> u8 puts everything in inner. Let's double check.
        let mut bits: u32 = 0;
        let (prefix, inner, suffix) =
            unsafe { std::slice::from_mut(&mut bits).align_to_mut::<u8>() };
        assert_eq!(prefix.len(), 0);
        assert_eq!(inner.len(), std::mem::size_of::<u32>());
        assert_eq!(suffix.len(), 0);

        let mut ev: MaybeUninit<libc::input_event> = MaybeUninit::uninit();
        let (prefix, inner, suffix) = unsafe { std::slice::from_mut(&mut ev).align_to_mut::<u8>() };
        assert_eq!(prefix.len(), 0);
        assert_eq!(inner.len(), std::mem::size_of::<libc::input_event>());
        assert_eq!(suffix.len(), 0);
    }
}
