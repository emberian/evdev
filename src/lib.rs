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
//! LEDs are currently lit on the device. As the application reads events, this state will be
//! updated, and it will be fully synchronized with the kernel if the stream drops any events.
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

// should really be cfg(target_os = "linux") and maybe also android?
#![cfg(unix)]

// has to be first for its macro
#[macro_use]
mod attribute_set;

mod constants;
mod device_state;
mod inputid;
pub mod raw_stream;
mod scancodes;
mod sync_stream;
mod sys;
pub mod uinput;

#[cfg(feature = "tokio")]
mod tokio_stream;

use std::os::unix::ffi::OsStrExt;
use std::time::{Duration, SystemTime};
use std::{fmt, io};

// pub use crate::constants::FFEffect::*;
pub use attribute_set::{AttributeSet, AttributeSetRef};
pub use constants::*;
pub use device_state::DeviceState;
pub use inputid::*;
pub use scancodes::*;
pub use sync_stream::*;

const EVENT_BATCH_SIZE: usize = 32;

/// A convenience mapping from an event `(type, code)` to an enumeration.
///
/// Note that this does not capture an event's value, just the type and code.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
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

/// A wrapped `libc::input_event` returned by the input device via the kernel.
///
/// `input_event` is a struct containing four fields:
/// - `time: timeval`
/// - `type_: u16`
/// - `code: u16`
/// - `value: s32`
///
/// The meaning of the "code" and "value" fields will depend on the underlying type of event.
#[derive(Copy, Clone)]
#[repr(transparent)]
pub struct InputEvent(libc::input_event);

impl InputEvent {
    /// Returns the timestamp associated with the event.
    #[inline]
    pub fn timestamp(&self) -> SystemTime {
        timeval_to_systime(&self.0.time)
    }

    /// Returns the type of event this describes, e.g. Key, Switch, etc.
    #[inline]
    pub fn event_type(&self) -> EventType {
        EventType(self.0.type_)
    }

    /// Returns the raw "code" field directly from input_event.
    #[inline]
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

    /// Returns the raw "value" field directly from input_event.
    ///
    /// For keys and switches the values 0 and 1 map to pressed and not pressed respectively.
    /// For axes, the values depend on the hardware and driver implementation.
    #[inline]
    pub fn value(&self) -> i32 {
        self.0.value
    }

    /// Create a new InputEvent. Only really useful for emitting events on virtual devices.
    pub fn new(type_: EventType, code: u16, value: i32) -> Self {
        InputEvent(libc::input_event {
            time: libc::timeval {
                tv_sec: 0,
                tv_usec: 0,
            },
            type_: type_.0,
            code,
            value,
        })
    }

    /// Create a new InputEvent with the time field set to "now" on the system clock.
    ///
    /// Note that this isn't usually necessary simply for emitting events on a virtual device, as
    /// even though [`InputEvent::new`] creates an `input_event` with the time field as zero,
    /// the kernel will update `input_event.time` when it emits the events to any programs reading
    /// the event "file".
    pub fn new_now(type_: EventType, code: u16, value: i32) -> Self {
        InputEvent(libc::input_event {
            time: systime_to_timeval(&SystemTime::now()),
            type_: type_.0,
            code,
            value,
        })
    }
}

impl From<libc::input_event> for InputEvent {
    fn from(raw: libc::input_event) -> Self {
        Self(raw)
    }
}

impl AsRef<libc::input_event> for InputEvent {
    fn as_ref(&self) -> &libc::input_event {
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
                let path = entry.path();
                let fname = path.file_name().unwrap();
                if fname.as_bytes().starts_with(b"event") {
                    if let Ok(dev) = Device::open(&path) {
                        return Some(dev);
                    }
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

pub(crate) fn nix_err(err: nix::Error) -> io::Error {
    match err {
        nix::Error::Sys(errno) => io::Error::from_raw_os_error(errno as i32),
        nix::Error::InvalidPath => io::Error::new(io::ErrorKind::InvalidInput, err),
        nix::Error::InvalidUtf8 => io::Error::new(io::ErrorKind::Other, err),
        // TODO: io::ErrorKind::NotSupported once stable
        nix::Error::UnsupportedOperation => io::Error::new(io::ErrorKind::Other, err),
    }
}

/// SAFETY: T must not have any padding or otherwise uninitialized bytes inside of it
pub(crate) unsafe fn cast_to_bytes<T: ?Sized>(mem: &T) -> &[u8] {
    std::slice::from_raw_parts(mem as *const T as *const u8, std::mem::size_of_val(mem))
}
