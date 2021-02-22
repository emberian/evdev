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
use std::ffi::CString;
use std::fs::File;
use std::fs::OpenOptions;
use std::mem::size_of;
use std::os::unix::{
    fs::OpenOptionsExt,
    io::{AsRawFd, RawFd},
};
use std::path::Path;
use std::time::SystemTime;

pub use crate::scancodes::*;
pub use crate::constants::FFEffect::*;
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
    pub key_vals: FixedBitSet,
    pub abs_vals: Vec<input_absinfo>,
    /// Set = switch enabled (closed)
    pub switch_vals: FixedBitSet,
    /// Set = LED lit
    pub led_vals: FixedBitSet,
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
    name: CString,
    phys: Option<CString>,
    uniq: Option<CString>,
    id: input_id,
    props: Props,
    driver_version: (u8, u8, u8),
    supported_keys: FixedBitSet,
    rel: RelativeAxis,
    abs: AbsoluteAxis,
    switch: Switch,
    led: Led,
    misc: Misc,
    ff: FixedBitSet,
    ff_stat: FFStatus,
    rep: Repeat,
    snd: Sound,
    pending_events: Vec<input_event>,
    // pending_events[last_seen..] is the events that have occurred since the last sync.
    last_seen: usize,
    state: DeviceState,
}

fn bus_name(x: u16) -> &'static str {
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

        if self.ty.contains(Types::SYNCHRONIZATION) {}

        if self.ty.contains(Types::KEY) {
            writeln!(f, "  Keys supported:")?;
            for key_idx in 0..self.supported_keys.len() {
                if self.supported_keys.contains(key_idx) {
                    writeln!(
                        f,
                        "    {:?} ({}index {})",
                        Key::new(key_idx as u32),
                        if self.state.key_vals.contains(key_idx) {
                            "pressed, "
                        } else {
                            ""
                        },
                        key_idx
                    )?;
                }
            }
        }
        if self.ty.contains(Types::RELATIVE) {
            writeln!(f, "  Relative Axes: {:?}", self.rel)?;
        }
        if self.ty.contains(Types::ABSOLUTE) {
            writeln!(f, "  Absolute Axes:")?;
            for idx in 0..AbsoluteAxis::MAX {
                let abs = AbsoluteAxis::from_bits_truncate(1 << idx);
                if self.abs.contains(abs) {
                    writeln!(
                        f,
                        "    {:?} ({:?}, index {})",
                        abs,
                        self.state.abs_vals[idx],
                        idx
                    )?;
                }
            }
        }
        if self.ty.contains(Types::MISC) {
            writeln!(f, "  Miscellaneous capabilities: {:?}", self.misc)?;
        }
        if self.ty.contains(Types::SWITCH) {
            writeln!(f, "  Switches:")?;
            for idx in 0..Switch::MAX {
                let sw = Switch::from_bits(1 << idx).unwrap();
                if self.switch.contains(sw) {
                    writeln!(
                        f,
                        "    {:?} ({:?}, index {})",
                        sw,
                        self.state.switch_vals[idx],
                        idx
                    )?;
                }
            }
        }
        if self.ty.contains(Types::LED) {
            writeln!(f, "  LEDs:")?;
            for idx in 0..Led::MAX {
                let led = Led::from_bits_truncate(1 << idx);
                if self.led.contains(led) {
                    writeln!(
                        f,
                        "    {:?} ({:?}, index {})",
                        led,
                        self.state.led_vals[idx],
                        idx
                    )?;
                }
            }
        }
        if self.ty.contains(Types::SOUND) {
            writeln!(f, "  Sound: {:?}", self.snd)?;
        }
        if self.ty.contains(Types::REPEAT) {
            writeln!(f, "  Repeats: {:?}", self.rep)?;
        }
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
    pub fn fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }

    pub fn events_supported(&self) -> Types {
        self.ty
    }

    pub fn name(&self) -> &CString {
        &self.name
    }

    pub fn physical_path(&self) -> &Option<CString> {
        &self.phys
    }

    pub fn unique_name(&self) -> &Option<CString> {
        &self.uniq
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

    pub fn keys_supported(&self) -> &FixedBitSet {
        &self.supported_keys
    }

    pub fn relative_axes_supported(&self) -> RelativeAxis {
        self.rel
    }

    pub fn absolute_axes_supported(&self) -> AbsoluteAxis {
        self.abs
    }

    pub fn switches_supported(&self) -> Switch {
        self.switch
    }

    pub fn leds_supported(&self) -> Led {
        self.led
    }

    pub fn misc_properties(&self) -> Misc {
        self.misc
    }

    pub fn repeats_supported(&self) -> Repeat {
        self.rep
    }

    pub fn sounds_supported(&self) -> Sound {
        self.snd
    }

    pub fn state(&self) -> &DeviceState {
        &self.state
    }

    pub fn open(path: &dyn AsRef<Path>) -> Result<Device, Error> {
        let mut options = OpenOptions::new();

        // Try to load read/write, then fall back to read-only.
        let file = options
            .read(true)
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(path)
            .or_else(|_| options.write(false).open(path))?;

        let mut dev = Device {
            file,
            ty: Types::empty(),
            name: CString::default(),
            phys: None,
            uniq: None,
            id: input_id_default(),
            props: Props::empty(),
            driver_version: (0, 0, 0),
            supported_keys: FixedBitSet::with_capacity(Key::MAX),
            rel: RelativeAxis::empty(),
            abs: AbsoluteAxis::empty(),
            switch: Switch::empty(),
            led: Led::empty(),
            misc: Misc::empty(),
            ff: FixedBitSet::with_capacity(FFEffect::MAX),
            ff_stat: FFStatus::empty(),
            rep: Repeat::empty(),
            snd: Sound::empty(),
            pending_events: Vec::with_capacity(64),
            last_seen: 0,
            state: DeviceState {
                timestamp: libc::timeval {
                    tv_sec: 0,
                    tv_usec: 0,
                },
                key_vals: FixedBitSet::with_capacity(Key::MAX),
                abs_vals: vec![],
                switch_vals: FixedBitSet::with_capacity(0x10),
                led_vals: FixedBitSet::with_capacity(0x10),
            },
        };

        // Sanity-check the FixedBitSet sizes. If they are not multiples of 8, odd things will happen.
        debug_assert!(dev.supported_keys.len() % 8 == 0);
        debug_assert!(dev.ff.len() % 8 == 0);
        debug_assert!(dev.state.key_vals.len() % 8 == 0);
        debug_assert!(dev.state.led_vals.len() % 8 == 0);

        let mut bits: u32 = 0;
        let mut bits64: u64 = 0;

        unsafe {
            let (_, bits_as_u8_slice, _) = std::slice::from_mut(&mut bits).align_to_mut();
            eviocgbit(dev.file.as_raw_fd(), 0, bits_as_u8_slice)?;
        }
        dev.ty = Types::from_bits(bits).expect("evdev: unexpected type bits! report a bug");

        dev.name =
            ioctl_get_cstring(eviocgname, dev.file.as_raw_fd()).unwrap_or_else(CString::default);
        dev.phys = ioctl_get_cstring(eviocgphys, dev.file.as_raw_fd());
        dev.uniq = ioctl_get_cstring(eviocguniq, dev.file.as_raw_fd());

        unsafe {
            eviocgid(dev.file.as_raw_fd(), &mut dev.id)?;
        }
        let mut driver_version: i32 = 0;
        unsafe {
            eviocgversion(dev.file.as_raw_fd(), &mut driver_version)?;
        }
        dev.driver_version = (
            ((driver_version >> 16) & 0xff) as u8,
            ((driver_version >> 8) & 0xff) as u8,
            (driver_version & 0xff) as u8,
        );

        unsafe {
            let (_, bits_as_u8_slice, _) = std::slice::from_mut(&mut bits).align_to_mut();
            eviocgprop(dev.file.as_raw_fd(), bits_as_u8_slice)?;
        } // FIXME: handle old kernel
        dev.props = Props::from_bits(bits).expect("evdev: unexpected prop bits! report a bug");

        if dev.ty.contains(Types::KEY) {
            unsafe {
                let key_slice = dev.supported_keys.as_mut_slice();
                let (_, supported_keys_as_u8_slice, _) = key_slice.align_to_mut();
                debug_assert!(supported_keys_as_u8_slice.len() == Key::MAX / 8);
                eviocgbit(
                    dev.file.as_raw_fd(),
                    Types::KEY.number(),
                    supported_keys_as_u8_slice,
                )?;
            }
        }

        if dev.ty.contains(Types::RELATIVE) {
            unsafe {
                let (_, bits_as_u8_slice, _) = std::slice::from_mut(&mut bits).align_to_mut();
                eviocgbit(
                    dev.file.as_raw_fd(),
                    Types::RELATIVE.number(),
                    bits_as_u8_slice,
                )?;
            }
            dev.rel =
                RelativeAxis::from_bits(bits).expect("evdev: unexpected rel bits! report a bug");
        }

        if dev.ty.contains(Types::ABSOLUTE) {
            unsafe {
                let (_, bits64_as_u8_slice, _) = std::slice::from_mut(&mut bits64).align_to_mut();
                eviocgbit(
                    dev.file.as_raw_fd(),
                    Types::ABSOLUTE.number(),
                    bits64_as_u8_slice,
                )?;
            }
            dev.abs =
                AbsoluteAxis::from_bits(bits64).expect("evdev: unexpected abs bits! report a bug");
            dev.state.abs_vals = vec![input_absinfo_default(); 0x3f];
        }

        if dev.ty.contains(Types::SWITCH) {
            unsafe {
                let (_, bits_as_u8_slice, _) = std::slice::from_mut(&mut bits).align_to_mut();
                eviocgbit(
                    dev.file.as_raw_fd(),
                    Types::SWITCH.number(),
                    bits_as_u8_slice,
                )?;
            }
            dev.switch =
                Switch::from_bits(bits).expect("evdev: unexpected switch bits! report a bug");
        }

        if dev.ty.contains(Types::LED) {
            unsafe {
                let (_, bits_as_u8_slice, _) = std::slice::from_mut(&mut bits).align_to_mut();
                eviocgbit(dev.file.as_raw_fd(), Types::LED.number(), bits_as_u8_slice)?;
            }
            dev.led = Led::from_bits(bits).expect("evdev: unexpected led bits! report a bug");
        }

        if dev.ty.contains(Types::MISC) {
            unsafe {
                let (_, bits_as_u8_slice, _) = std::slice::from_mut(&mut bits).align_to_mut();
                eviocgbit(dev.file.as_raw_fd(), Types::MISC.number(), bits_as_u8_slice)?;
            }
            dev.misc = Misc::from_bits(bits).expect("evdev: unexpected misc bits! report a bug");
        }

        //unsafe { eviocgbit(dev.file.as_raw_fd(), ffs(FORCEFEEDBACK.bits()), 0x7f, bits_as_u8_slice)?; }

        if dev.ty.contains(Types::SOUND) {
            unsafe {
                let (_, bits_as_u8_slice, _) = std::slice::from_mut(&mut bits).align_to_mut();
                eviocgbit(
                    dev.file.as_raw_fd(),
                    Types::SOUND.number(),
                    bits_as_u8_slice,
                )?;
            }
            dev.snd = Sound::from_bits(bits).expect("evdev: unexpected sound bits! report a bug");
        }

        dev.sync_state()?;

        Ok(dev)
    }

    /// Synchronize the `Device` state with the kernel device state.
    ///
    /// If there is an error at any point, the state will not be synchronized completely.
    pub fn sync_state(&mut self) -> Result<(), Error> {
        if self.ty.contains(Types::KEY) {
            unsafe {
                let key_slice = self.state.key_vals.as_mut_slice();
                let (_, key_vals_as_u8_slice, _) = key_slice.align_to_mut();
                eviocgkey(self.file.as_raw_fd(), key_vals_as_u8_slice)?;
            }
        }
        if self.ty.contains(Types::ABSOLUTE) {
            for idx in 0..AbsoluteAxis::MAX {
                let abs = 1 << idx;
                // ignore multitouch, we'll handle that later.
                //
                // handling later removed. not sure what the intention of "handling that later" was
                // the abs data seems to be fine (tested ABS_MT_POSITION_X/Y)
                if self.abs.bits() & abs != 0 {
                    unsafe {
                        eviocgabs(
                            self.file.as_raw_fd(),
                            idx as u32,
                            &mut self.state.abs_vals[idx],
                        )?;
                    }
                }
            }
        }
        if self.ty.contains(Types::SWITCH) {
            unsafe {
                let switch_slice = self.state.switch_vals.as_mut_slice();
                let (_, switch_vals_as_u8_slice, _) = switch_slice.align_to_mut();
                eviocgsw(self.file.as_raw_fd(), switch_vals_as_u8_slice)?;
            }
        }
        if self.ty.contains(Types::LED) {
            unsafe {
                let led_slice = self.state.led_vals.as_mut_slice();
                let (_, led_vals_as_u8_slice, _) = led_slice.align_to_mut();
                eviocgled(self.file.as_raw_fd(), led_vals_as_u8_slice)?;
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

        if self.ty.contains(Types::KEY) {
            for key_idx in 0..self.supported_keys.len() {
                if self.supported_keys.contains(key_idx)
                    && old_state.key_vals[key_idx] != self.state.key_vals[key_idx]
                {
                    self.pending_events.push(raw::input_event {
                        time,
                        type_: Types::KEY.number(),
                        code: key_idx as u16,
                        value: if self.state.key_vals[key_idx] { 1 } else { 0 },
                    });
                }
            }
        }
        if self.ty.contains(Types::ABSOLUTE) {
            for idx in 0..AbsoluteAxis::MAX {
                let abs = AbsoluteAxis::from_bits_truncate(1 << idx);
                if self.abs.contains(abs)
                    && old_state.abs_vals[idx] != self.state.abs_vals[idx]
                {
                    self.pending_events.push(raw::input_event {
                        time,
                        type_: Types::ABSOLUTE.number(),
                        code: idx as u16,
                        value: self.state.abs_vals[idx].value,
                    });
                }
            }
        }
        if self.ty.contains(Types::SWITCH) {
            for idx in 0..Switch::MAX {
                let sw = Switch::from_bits(1 << idx).unwrap();
                if self.switch.contains(sw)
                    && old_state.switch_vals[idx] != self.state.switch_vals[idx]
                {
                    self.pending_events.push(raw::input_event {
                        time,
                        type_: Types::SWITCH.number(),
                        code: idx as u16,
                        value: if self.state.switch_vals[idx] {
                            1
                        } else {
                            0
                        },
                    });
                }
            }
        }
        if self.ty.contains(Types::LED) {
            for idx in 0..Led::MAX {
                let led = Led::from_bits_truncate(1 << idx);
                if self.led.contains(led)
                    && old_state.led_vals[idx] != self.state.led_vals[idx]
                {
                    self.pending_events.push(raw::input_event {
                        time,
                        type_: Types::LED.number(),
                        code: idx as u16,
                        value: if self.state.led_vals[idx] {
                            1
                        } else {
                            0
                        },
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
        let buf = &mut self.pending_events;
        loop {
            buf.reserve(20);
            // TODO: use spare_capacity_mut or split_at_spare_mut when they stabilize
            let pre_len = buf.len();
            let capacity = buf.capacity();
            let (_, unsafe_buf_slice, _) =
                unsafe { buf.get_unchecked_mut(pre_len..capacity).align_to_mut() };

            match nix::unistd::read(self.file.as_raw_fd(), unsafe_buf_slice) {
                Ok(bytes_read) => unsafe {
                    buf.set_len(pre_len + (bytes_read / size_of::<raw::input_event>()));
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

        Ok(RawEvents(self))
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
/// an empty vector or omits the devices that could not be opened.
pub fn enumerate() -> Vec<Device> {
    let mut res = Vec::new();
    if let Ok(dir) = std::fs::read_dir("/dev/input") {
        for entry in dir {
            if let Ok(entry) = entry {
                if let Ok(dev) = Device::open(&entry.path()) {
                    res.push(dev)
                }
            }
        }
    }
    res
}

/// A safe Rust version of clock_gettime against CLOCK_REALTIME
fn into_timeval(time: &SystemTime) -> Result<libc::timeval, std::time::SystemTimeError> {
    let now_duration = time.duration_since(SystemTime::UNIX_EPOCH)?;

    Ok(libc::timeval {
        tv_sec: now_duration.as_secs() as libc::time_t,
        tv_usec: now_duration.subsec_micros() as libc::suseconds_t,
    })
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
