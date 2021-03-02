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
//!
//! For demonstrations of how to use this library in blocking, nonblocking, and async (tokio) modes,
//! please reference the "examples" directory.

#![cfg(any(unix, target_os = "android"))]
#![allow(non_camel_case_types)]

// has to be first for its macro
#[macro_use]
mod attribute_set;

mod constants;
pub mod raw_events;
mod scancodes;
mod sync_device;
mod sys;

#[cfg(feature = "tokio")]
mod tokio_stream;

use std::time::{Duration, SystemTime};
use std::{fmt, io};

// pub use crate::constants::FFEffect::*;
pub use crate::attribute_set::AttributeSet;
pub use crate::constants::*;
pub use crate::scancodes::*;
pub use crate::sync_device::*;
#[cfg(feature = "tokio")]
pub use crate::tokio_stream::EventStream;

const fn bit_elts<T>(bits: usize) -> usize {
    let width = std::mem::size_of::<T>() * 8;
    bits / width + (bits % width != 0) as usize
}
// TODO: this is a replacement for BitArr!(for Key::COUNT, in u8), since const generics aren't stable
// and the BitView impls for arrays only goes up to 64
const KEY_ARRAY_LEN: usize = bit_elts::<u8>(Key::COUNT);
type KeyArray = [u8; KEY_ARRAY_LEN];
const KEY_ARRAY_INIT: KeyArray = [0; KEY_ARRAY_LEN];

const EVENT_BATCH_SIZE: usize = 32;

//impl Device {
//    #[cfg(feature = "tokio")]
//    /// Return a `futures::stream` asynchronous stream of `InputEvent` compatible with Tokio.
//    ///
//    /// The stream does NOT compensate for SYN_DROPPED events and will not update internal cached
//    /// state.
//    /// The Tokio runtime is expected to keep up with typical event rates.
//    /// This operation consumes the Device.
//    pub fn into_event_stream_no_sync(self) -> io::Result<tokio_stream::EventStream> {
//        tokio_stream::EventStream::new(self)
//    }

//    /// Returns the *cached* state of the device.
//    ///
//    /// Pulling updates via `fetch_events` or manually invoking `sync_state` will refresh the cache.
//    pub fn state(&self) -> &DeviceState {
//        &self.state
//    }

//    /// Synchronize the `Device` state with the kernel device state.
//    ///
//    /// If there is an error at any point, the state will not be synchronized completely.
//    pub fn sync_state(&mut self) -> io::Result<()> {
//        let fd = self.as_raw_fd();
//        if let Some(key_vals) = &mut self.state.key_vals {
//            unsafe { sys::eviocgkey(fd, &mut key_vals[..]).map_err(nix_err)? };
//        }

//        if let (Some(supported_abs), Some(abs_vals)) =
//            (self.supported_absolute, &mut self.state.abs_vals)
//        {
//            for idx in supported_abs.iter_ones() {
//                // ignore multitouch, we'll handle that later.
//                //
//                // handling later removed. not sure what the intention of "handling that later" was
//                // the abs data seems to be fine (tested ABS_MT_POSITION_X/Y)
//                unsafe { sys::eviocgabs(fd, idx as u32, &mut abs_vals[idx]).map_err(nix_err)? };
//            }
//        }

//        if let Some(switch_vals) = &mut self.state.switch_vals {
//            unsafe { sys::eviocgsw(fd, switch_vals.as_mut_raw_slice()).map_err(nix_err)? };
//        }

//        if let Some(led_vals) = &mut self.state.led_vals {
//            unsafe { sys::eviocgled(fd, led_vals.as_mut_raw_slice()).map_err(nix_err)? };
//        }

//        Ok(())
//    }

//    /// Do SYN_DROPPED synchronization, and compensate for missing events by inserting events into
//    /// the stream which, when applied to any state being kept outside of this `Device`, will
//    /// synchronize it with the kernel state.
//    fn compensate_dropped(&mut self) -> io::Result<()> {
//        let mut drop_from = None;
//        for (idx, event) in self.pending_events.iter().enumerate() {
//            if event.type_ == EventType::SYNCHRONIZATION.0
//                && event.code == Synchronization::SYN_DROPPED.0
//            {
//                drop_from = Some(idx);
//                break;
//            }
//        }
//        // FIXME: see if we can *not* drop EV_REL events. EV_REL doesn't have any state, so
//        // dropping its events isn't really helping much.
//        if let Some(idx) = drop_from {
//            // look for the nearest SYN_REPORT before the SYN_DROPPED, remove everything after it.
//            let mut prev_report = 0; // (if there's no previous SYN_REPORT, then the entire vector is bogus)
//            for (idx, event) in self.pending_events.iter().take(idx).enumerate().rev() {
//                if event.type_ == EventType::SYNCHRONIZATION.0
//                    && event.code == Synchronization::SYN_REPORT.0
//                {
//                    prev_report = idx;
//                    break;
//                }
//            }
//            self.pending_events.truncate(prev_report);
//        } else {
//            return Ok(());
//        }

//        // Alright, pending_events is in a sane state. Now, let's sync the local state. We will
//        // create a phony packet that contains deltas from the previous device state to the current
//        // device state.
//        let old_state = self.state.clone();
//        self.sync_state()?;

//        let time = systime_to_timeval(&SystemTime::now());

//        if let (Some(supported_keys), Some(key_vals)) =
//            (&self.supported_keys, self.state.key_vals())
//        {
//            let supported_keys =
//                AttributeSet::new(BitSlice::from_slice(&supported_keys[..]).unwrap());
//            let old_vals = old_state.key_vals();
//            for key in supported_keys.iter() {
//                if old_vals.map(|v| v.contains(key)) != Some(key_vals.contains(key)) {
//                    self.pending_events.push_back(libc::input_event {
//                        time,
//                        type_: EventType::KEY.0 as _,
//                        code: key.code() as u16,
//                        value: if key_vals.contains(key) { 1 } else { 0 },
//                    });
//                }
//            }
//        }

//        if let (Some(supported_abs), Some(abs_vals)) =
//            (self.supported_absolute, &self.state.abs_vals)
//        {
//            for idx in supported_abs.iter_ones() {
//                if old_state.abs_vals.as_ref().map(|v| v[idx]) != Some(abs_vals[idx]) {
//                    self.pending_events.push_back(libc::input_event {
//                        time,
//                        type_: EventType::ABSOLUTE.0 as _,
//                        code: idx as u16,
//                        value: abs_vals[idx].value,
//                    });
//                }
//            }
//        }

//        if let (Some(supported_switch), Some(switch_vals)) =
//            (self.supported_switch, &self.state.switch_vals)
//        {
//            for idx in supported_switch.iter_ones() {
//                if old_state.switch_vals.as_ref().map(|v| v[idx]) != Some(switch_vals[idx]) {
//                    self.pending_events.push_back(libc::input_event {
//                        time,
//                        type_: EventType::SWITCH.0 as _,
//                        code: idx as u16,
//                        value: if switch_vals[idx] { 1 } else { 0 },
//                    });
//                }
//            }
//        }

//        if let (Some(supported_led), Some(led_vals)) = (self.supported_led, &self.state.led_vals) {
//            for idx in supported_led.iter_ones() {
//                if old_state.led_vals.as_ref().map(|v| v[idx]) != Some(led_vals[idx]) {
//                    self.pending_events.push_back(libc::input_event {
//                        time,
//                        type_: EventType::LED.0 as _,
//                        code: idx as u16,
//                        value: if led_vals[idx] { 1 } else { 0 },
//                    });
//                }
//            }
//        }

//        self.pending_events.push_back(libc::input_event {
//            time,
//            type_: EventType::SYNCHRONIZATION.0 as _,
//            code: Synchronization::SYN_REPORT.0,
//            value: 0,
//        });
//        Ok(())
//    }
//}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
/// A convenience mapping from an event `(type, code)` to an enumeration.
///
/// Note that this does not capture an event's value, just the type and code.
pub enum InputEventKind {
    Synchronization(Synchronization),
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
/// A wrapped `libc::input_event` returned by the input device via the kernel.
///
/// `input_event` is a struct containing four fields:
/// - `time: timeval`
/// - `type_: u16`
/// - `code: u16`
/// - `value: s32`
///
/// The meaning of the "code" and "value" fields will depend on the underlying type of event.
pub struct InputEvent(libc::input_event);

impl InputEvent {
    #[inline]
    /// Returns the timestamp associated with the event.
    pub fn timestamp(&self) -> SystemTime {
        timeval_to_systime(&self.0.time)
    }

    #[inline]
    /// Returns the type of event this describes, e.g. Key, Switch, etc.
    pub fn event_type(&self) -> EventType {
        EventType(self.0.type_)
    }

    #[inline]
    /// Returns the raw "code" field directly from input_event.
    pub fn code(&self) -> u16 {
        self.0.code
    }

    /// A convenience function to return `self.code()` wrapped in a certain newtype determined by
    /// the type of this event.
    ///
    /// This is useful if you want to match events by specific key codes or axes. Note that this
    /// does not capture the event value, just the type and code.
    #[inline]
    pub fn kind(&self) -> InputEventKind {
        let code = self.code();
        match self.event_type() {
            EventType::SYNCHRONIZATION => InputEventKind::Synchronization(Synchronization(code)),
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
    /// Returns the raw "value" field directly from input_event.
    ///
    /// For keys and switches the values 0 and 1 map to pressed and not pressed respectively.
    /// For axes, the values depend on the hardware and driver implementation.
    pub fn value(&self) -> i32 {
        self.0.value
    }
}

impl From<libc::input_event> for InputEvent {
    fn from(raw: libc::input_event) -> Self {
        Self(raw)
    }
}

impl<'a> Into<&'a libc::input_event> for &'a InputEvent {
    fn into(self) -> &'a libc::input_event {
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

fn nix_err(err: nix::Error) -> io::Error {
    match err {
        nix::Error::Sys(errno) => io::Error::from_raw_os_error(errno as i32),
        nix::Error::InvalidPath => io::Error::new(io::ErrorKind::InvalidInput, err),
        nix::Error::InvalidUtf8 => io::Error::new(io::ErrorKind::Other, err),
        // TODO: io::ErrorKind::NotSupported once stable
        nix::Error::UnsupportedOperation => io::Error::new(io::ErrorKind::Other, err),
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
