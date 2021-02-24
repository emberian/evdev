//! Linux event device handling.
//!
//! The Linux kernel's "evdev" subsystem exposes input devices to userspace in a generic,
//! consistent way. I'll try to explain the device model as completely as possible. The upstream
//! kernel documentation is split across two files:
//!
//! - https://www.kernel.org/doc/Documentation/input/event-codes.txt
//! - https://www.kernel.org/doc/Documentation/input/multi-touch-protocol.txt
//!
//! Devices can expose a few different kinds of events, specified by the `Types` bitflag. Each
//! event type (except for RELATIVE and SYNCHRONIZATION) also has some associated state. See the documentation for
//! `Types` on what each type corresponds to.
//!
//! This state can be queried. For example, the `DeviceState::led_vals` field will tell you which
//! LEDs are currently lit on the device. This state is not automatically synchronized with the
//! kernel. However, as the application reads events, this state will be updated if the event is
//! newer than the state timestamp (maintained internally).  Additionally, you can call
//! `Device::sync_state` to explicitly synchronize with the kernel state.
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
//! It is recommended that you dedicate a thread to processing input events, or use epoll with the
//! fd returned by `Device::fd` to process events when they are ready.

#![cfg(any(unix, target_os = "android"))]
#![allow(non_camel_case_types)]

mod constants;
pub mod raw;
mod scancodes;

use fixedbitset::FixedBitSet;
use std::fs::File;
use std::fs::OpenOptions;
use std::mem;
use std::os::unix::{
    fs::OpenOptionsExt,
    io::{AsRawFd, RawFd},
};
use std::path::Path;
use std::time::SystemTime;
use std::{ffi::CString, mem::MaybeUninit};

pub use crate::constants::FFEffect::*;
pub use crate::scancodes::*;
pub use crate::Synchronization::*;

pub use crate::constants::*;
use crate::raw::*;

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

macro_rules! impl_number {
    ($($t:ident),*) => {
        $(impl $t {
            /// Given a bitflag with only a single flag set, returns the event code corresponding to that
            /// event. If multiple flags are set, the one with the most significant bit wins. In debug
            /// mode,
            #[inline(always)]
            pub fn number<T: num_traits::FromPrimitive>(&self) -> T {
                let val = self.bits().trailing_zeros();
                debug_assert!(self.bits() == 1 << val, "{:?} ought to have only one flag set to be used with .number()", self);
                T::from_u32(val).unwrap()
            }
        })*
    }
}

impl_number!(
    Types,
    Props,
    RelativeAxis,
    AbsoluteAxis,
    Switch,
    Led,
    Misc,
    FFStatus,
    Repeat,
    Sound
);

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

#[derive(Debug, Clone)]
pub struct DeviceState {
    /// The state corresponds to kernel state at this timestamp.
    pub timestamp: libc::timeval,
    /// Set = key pressed
    pub key_vals: Option<FixedBitSet>,
    pub abs_vals: Option<Vec<input_absinfo>>,
    /// Set = switch enabled (closed)
    pub switch_vals: Option<FixedBitSet>,
    /// Set = LED lit
    pub led_vals: Option<FixedBitSet>,
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
    ty: Types,
    name: Option<String>,
    phys: Option<String>,
    uniq: Option<String>,
    id: input_id,
    props: Props,
    driver_version: (u8, u8, u8),
    supported_keys: Option<FixedBitSet>,
    supported_relative: Option<RelativeAxis>,
    supported_absolute: Option<AbsoluteAxis>,
    supported_switch: Option<Switch>,
    supported_led: Option<Led>,
    supported_misc: Option<Misc>,
    // ff: Option<FixedBitSet>,
    // ff_stat: Option<FFStatus>,
    // rep: Option<Repeat>,
    supported_snd: Option<Sound>,
    pending_events: Vec<input_event>,
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

impl std::fmt::Display for Device {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        writeln!(f, "{:?}", self.name)?;
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
        writeln!(f, "  Vendor: 0x{:x}", self.id.vendor)?;
        writeln!(f, "  Product: 0x{:x}", self.id.product)?;
        writeln!(f, "  Version: 0x{:x}", self.id.version)?;
        writeln!(f, "  Properties: {:?}", self.props)?;

        if let (Some(supported_keys), Some(key_vals)) = (&self.supported_keys, &self.state.key_vals)
        {
            writeln!(f, "  Keys supported:")?;
            for key_idx in 0..supported_keys.len() {
                if supported_keys.contains(key_idx) {
                    writeln!(
                        f,
                        "    {:?} ({}index {})",
                        Key::new(key_idx as u32),
                        if key_vals.contains(key_idx) {
                            "pressed, "
                        } else {
                            ""
                        },
                        key_idx
                    )?;
                }
            }
        }

        if let Some(supported_relative) = self.supported_relative {
            writeln!(f, "  Relative Axes: {:?}", supported_relative)?;
        }

        if let (Some(supported_abs), Some(abs_vals)) =
            (self.supported_absolute, &self.state.abs_vals)
        {
            writeln!(f, "  Absolute Axes:")?;
            for idx in 0..AbsoluteAxis::MAX {
                let abs = AbsoluteAxis::from_bits_truncate(1 << idx);
                if supported_abs.contains(abs) {
                    writeln!(f, "    {:?} ({:?}, index {})", abs, abs_vals[idx], idx)?;
                }
            }
        }

        if let Some(supported_misc) = self.supported_misc {
            writeln!(f, "  Miscellaneous capabilities: {:?}", supported_misc)?;
        }

        if let (Some(supported_switch), Some(switch_vals)) =
            (self.supported_switch, &self.state.switch_vals)
        {
            writeln!(f, "  Switches:")?;
            for idx in 0..Switch::MAX {
                let sw = Switch::from_bits(1 << idx).unwrap();
                if supported_switch.contains(sw) {
                    writeln!(f, "    {:?} ({:?}, index {})", sw, switch_vals[idx], idx)?;
                }
            }
        }

        if let (Some(supported_led), Some(led_vals)) = (self.supported_led, &self.state.led_vals) {
            writeln!(f, "  LEDs:")?;
            for idx in 0..Led::MAX {
                let led = Led::from_bits_truncate(1 << idx);
                if supported_led.contains(led) {
                    writeln!(f, "    {:?} ({:?}, index {})", led, led_vals[idx], idx)?;
                }
            }
        }

        if let Some(supported_snd) = self.supported_snd {
            writeln!(f, "  Sound: {:?}", supported_snd)?;
        }

        // if let Some(rep) = self.rep {
        //     writeln!(f, "  Repeats: {:?}", rep)?;
        // }

        if self.ty.contains(Types::FORCEFEEDBACK) {
            writeln!(f, "  Force Feedback supported")?;
        }

        if self.ty.contains(Types::POWER) {
            writeln!(f, "  Power supported")?;
        }

        if self.ty.contains(Types::FORCEFEEDBACKSTATUS) {
            writeln!(f, "  Force Feedback status supported")?;
        }

        Ok(())
    }
}

impl Device {
    pub fn events_supported(&self) -> Types {
        self.ty
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

    pub fn input_id(&self) -> input_id {
        self.id
    }

    pub fn properties(&self) -> Props {
        self.props
    }

    pub fn driver_version(&self) -> (u8, u8, u8) {
        self.driver_version
    }

    pub fn keys_supported(&self) -> &Option<FixedBitSet> {
        &self.supported_keys
    }

    pub fn relative_axes_supported(&self) -> Option<RelativeAxis> {
        self.supported_relative
    }

    pub fn absolute_axes_supported(&self) -> Option<AbsoluteAxis> {
        self.supported_absolute
    }

    pub fn switches_supported(&self) -> Option<Switch> {
        self.supported_switch
    }

    pub fn leds_supported(&self) -> Option<Led> {
        self.supported_led
    }

    pub fn misc_properties(&self) -> Option<Misc> {
        self.supported_misc
    }

    // pub fn repeats_supported(&self) -> Option<Repeat> {
    //     self.rep
    // }

    pub fn sounds_supported(&self) -> Option<Sound> {
        self.supported_snd
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

        let mut ty = 0;
        unsafe { eviocgbit_type(file.as_raw_fd(), &mut ty)? };
        let ty = Types::from_bits(ty).expect("evdev: unexpected type bits! report a bug");

        let name = ioctl_get_cstring(eviocgname, file.as_raw_fd())
            .map(|s| s.to_string_lossy().into_owned());
        let phys = ioctl_get_cstring(eviocgphys, file.as_raw_fd())
            .map(|s| s.to_string_lossy().into_owned());
        let uniq = ioctl_get_cstring(eviocguniq, file.as_raw_fd())
            .map(|s| s.to_string_lossy().into_owned());

        let id = unsafe {
            let mut id = MaybeUninit::uninit();
            eviocgid(file.as_raw_fd(), id.as_mut_ptr())?;
            id.assume_init()
        };
        let mut driver_version: i32 = 0;
        unsafe {
            eviocgversion(file.as_raw_fd(), &mut driver_version)?;
        }
        let driver_version = (
            ((driver_version >> 16) & 0xff) as u8,
            ((driver_version >> 8) & 0xff) as u8,
            (driver_version & 0xff) as u8,
        );

        let mut props = 0;
        unsafe {
            eviocgprop(file.as_raw_fd(), &mut props)?;
        } // FIXME: handle old kernel
        let props = Props::from_bits(props).expect("evdev: unexpected prop bits! report a bug");

        let mut state = DeviceState::default();

        let supported_keys = if ty.contains(Types::KEY) {
            let mut supported_keys = FixedBitSet::with_capacity(Key::MAX);
            debug_assert!(supported_keys.len() % 8 == 0);
            let key_slice = supported_keys.as_mut_slice();
            unsafe {
                let (_, supported_keys_as_u8_slice, _) = key_slice.align_to_mut();
                debug_assert!(supported_keys_as_u8_slice.len() == Key::MAX / 8);
                eviocgbit_key(file.as_raw_fd(), supported_keys_as_u8_slice)?;
            }
            let key_vals = FixedBitSet::with_capacity(Key::MAX);
            debug_assert!(key_vals.len() % 8 == 0);
            state.key_vals = Some(key_vals);

            Some(supported_keys)
        } else {
            None
        };

        let supported_relative = if ty.contains(Types::RELATIVE) {
            let mut rel = 0;
            unsafe { eviocgbit_relative(file.as_raw_fd(), &mut rel)? };
            Some(RelativeAxis::from_bits(rel).expect("evdev: unexpected rel bits! report a bug"))
        } else {
            None
        };

        let supported_absolute = if ty.contains(Types::ABSOLUTE) {
            let mut abs = 0;
            unsafe { eviocgbit_absolute(file.as_raw_fd(), &mut abs)? };
            state.abs_vals = Some(vec![input_absinfo_default(); 0x3f]);
            Some(AbsoluteAxis::from_bits(abs).expect("evdev: unexpected abs bits! report a bug"))
        } else {
            None
        };

        let supported_switch = if ty.contains(Types::SWITCH) {
            let mut switch = 0;
            unsafe { eviocgbit_switch(file.as_raw_fd(), &mut switch)? };
            state.switch_vals = Some(FixedBitSet::with_capacity(0x10));

            Some(Switch::from_bits(switch).expect("evdev: unexpected switch bits! report a bug"))
        } else {
            None
        };

        let supported_led = if ty.contains(Types::LED) {
            let mut led = 0;
            unsafe { eviocgbit_led(file.as_raw_fd(), &mut led)? };
            let led_vals = FixedBitSet::with_capacity(0x10);
            debug_assert!(led_vals.len() % 8 == 0);
            state.led_vals = Some(led_vals);

            Some(Led::from_bits(led).expect("evdev: unexpected led bits! report a bug"))
        } else {
            None
        };

        let supported_misc = if ty.contains(Types::MISC) {
            let mut misc = 0;
            unsafe { eviocgbit_misc(file.as_raw_fd(), &mut misc)? };
            Some(Misc::from_bits(misc).expect("evdev: unexpected misc bits! report a bug"))
        } else {
            None
        };

        //unsafe { eviocgbit(file.as_raw_fd(), ffs(FORCEFEEDBACK.bits()), 0x7f, bits_as_u8_slice)?; }

        let supported_snd = if ty.contains(Types::SOUND) {
            let mut snd = 0;
            unsafe { eviocgbit_sound(file.as_raw_fd(), &mut snd)? };
            Some(Sound::from_bits(snd).expect("evdev: unexpected sound bits! report a bug"))
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
            unsafe {
                let key_slice = key_vals.as_mut_slice();
                let (_, key_vals_as_u8_slice, _) = key_slice.align_to_mut();
                eviocgkey(fd, key_vals_as_u8_slice)?;
            }
        }

        if let (Some(supported_abs), Some(abs_vals)) =
            (self.supported_absolute, &mut self.state.abs_vals)
        {
            for idx in 0..AbsoluteAxis::MAX {
                let abs = AbsoluteAxis::from_bits_truncate(1 << idx);
                // ignore multitouch, we'll handle that later.
                //
                // handling later removed. not sure what the intention of "handling that later" was
                // the abs data seems to be fine (tested ABS_MT_POSITION_X/Y)
                if supported_abs.contains(abs) {
                    unsafe {
                        eviocgabs(fd, idx as u32, &mut abs_vals[idx])?;
                    }
                }
            }
        }

        if let Some(switch_vals) = &mut self.state.switch_vals {
            unsafe {
                let switch_slice = switch_vals.as_mut_slice();
                let (_, switch_vals_as_u8_slice, _) = switch_slice.align_to_mut();
                eviocgsw(fd, switch_vals_as_u8_slice)?;
            }
        }

        if let Some(led_vals) = &mut self.state.led_vals {
            unsafe {
                let led_slice = led_vals.as_mut_slice();
                let (_, led_vals_as_u8_slice, _) = led_slice.align_to_mut();
                eviocgled(fd, led_vals_as_u8_slice)?;
            }
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

        let time = into_timeval(&SystemTime::now()).unwrap();

        if let (Some(supported_keys), Some(key_vals)) = (&self.supported_keys, &self.state.key_vals)
        {
            for key_idx in 0..supported_keys.len() {
                if supported_keys.contains(key_idx)
                    && old_state.key_vals.as_ref().map(|v| v[key_idx]) != Some(key_vals[key_idx])
                {
                    self.pending_events.push(raw::input_event {
                        time,
                        type_: Types::KEY.number(),
                        code: key_idx as u16,
                        value: if key_vals[key_idx] { 1 } else { 0 },
                    });
                }
            }
        }

        if let (Some(supported_abs), Some(abs_vals)) =
            (self.supported_absolute, &self.state.abs_vals)
        {
            for idx in 0..AbsoluteAxis::MAX {
                let abs = AbsoluteAxis::from_bits_truncate(1 << idx);
                if supported_abs.contains(abs)
                    && old_state.abs_vals.as_ref().map(|v| v[idx]) != Some(abs_vals[idx])
                {
                    self.pending_events.push(raw::input_event {
                        time,
                        type_: Types::ABSOLUTE.number(),
                        code: idx as u16,
                        value: abs_vals[idx].value,
                    });
                }
            }
        }

        if let (Some(supported_switch), Some(switch_vals)) =
            (self.supported_switch, &self.state.switch_vals)
        {
            for idx in 0..Switch::MAX {
                let sw = Switch::from_bits(1 << idx).unwrap();
                if supported_switch.contains(sw)
                    && old_state.switch_vals.as_ref().map(|v| v[idx]) != Some(switch_vals[idx])
                {
                    self.pending_events.push(raw::input_event {
                        time,
                        type_: Types::SWITCH.number(),
                        code: idx as u16,
                        value: if switch_vals[idx] { 1 } else { 0 },
                    });
                }
            }
        }

        if let (Some(supported_led), Some(led_vals)) = (self.supported_led, &self.state.led_vals) {
            for idx in 0..Led::MAX {
                let led = Led::from_bits_truncate(1 << idx);
                if supported_led.contains(led)
                    && old_state.led_vals.as_ref().map(|v| v[idx]) != Some(led_vals[idx])
                {
                    self.pending_events.push(raw::input_event {
                        time,
                        type_: Types::LED.number(),
                        code: idx as u16,
                        value: if led_vals[idx] { 1 } else { 0 },
                    });
                }
            }
        }

        self.pending_events.push(raw::input_event {
            time,
            type_: Types::SYNCHRONIZATION.number(),
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
                        pre_len + (bytes_read as usize / mem::size_of::<raw::input_event>()),
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
    type Item = raw::input_event;

    #[inline(always)]
    fn next(&mut self) -> Option<raw::input_event> {
        self.0.pending_events.pop()
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
fn into_timeval(time: &SystemTime) -> Result<libc::timeval, std::time::SystemTimeError> {
    let now_duration = time.duration_since(SystemTime::UNIX_EPOCH)?;

    Ok(libc::timeval {
        tv_sec: now_duration.as_secs() as libc::time_t,
        tv_usec: now_duration.subsec_micros() as libc::suseconds_t,
    })
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
