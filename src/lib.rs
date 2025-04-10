//! Linux event device handling.
//!
//! The Linux kernel's "evdev" subsystem exposes input devices to userspace in a generic,
//! consistent way. I'll try to explain the device model as completely as possible. The upstream
//! kernel documentation is split across two files:
//!
//! - <https://www.kernel.org/doc/Documentation/input/event-codes.txt>
//! - <https://www.kernel.org/doc/Documentation/input/multi-touch-protocol.txt>
//!
//! The `evdev` kernel system exposes input devices as character devices in `/dev/input`,
//! typically `/dev/input/eventX` where `X` is an integer.
//! Userspace applications can use `ioctl` system calls to interact with these devices.
//! Libraries such as this one abstract away the low level calls to provide a high level
//! interface.
//!
//! Applications can interact with `uinput` by writing to `/dev/uinput` to create virtual
//! devices and send events to the virtual devices.
//! Virtual devices are created in `/sys/devices/virtual/input`.
//!
//! # Devices
//!
//! Devices can be opened directly via their path:
//! ```no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use evdev::Device;
//! let device = Device::open("/dev/input/event0")?;
//! # Ok(())
//! # }
//! ```
//! This approach requires the calling process to have the appropriate privileges to
//! open the device node (typically this requires running as root user).
//! Alternatively a device can be created from an already open file descriptor. This approach
//! is useful where the file descriptor is provided by an external privileged process
//! (e.g. systemd's logind):
//!
//! ```no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use evdev::Device;
//! use std::fs::File;
//! use std::os::fd::OwnedFd;
//! let f = File::open("/dev/input/event0")?;
//! let fd = OwnedFd::from(f);
//! let device = Device::from_fd(fd)?;
//! # Ok(())
//! # }
//! ```
//!
//! # Input Events
//!
//! Devices emit events, represented by the [`InputEvent`] struct.
//! A input event has three main fields: event [type](InputEvent::event_type), [code](InputEvent::code)
//! and [value](InputEvent::value)
//!
//! The kernel documentation specifies different event types, reperesented by the [`EventType`] struct.
//! Each device can support a subset of those types. See [`Device::supported_events()`].
//! For each of the known event types there is a new-type wrapper around [`InputEvent`]  
//! in [`event_variants`] see the module documenation for more info about those.
//!
//! For most event types the kernel documentation also specifies a set of codes, represented by a new-type
//! e.g. [`KeyCode`]. The individual codes of a [`EventType`] that a device supports can be retrieved
//! through the `Device::supported_*()` methods, e.g. [`Device::supported_keys()`]:
//! ```no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use evdev::{Device, KeyCode};
//! let device = Device::open("/dev/input/event0")?;
//! // check if the device has an ENTER key
//! if device.supported_keys().map_or(false, |keys| keys.contains(KeyCode::KEY_ENTER)) {
//!     println!("are you prepared to ENTER the world of evdev?");
//! } else {
//!     println!(":(");
//! }
//! # Ok(())
//! # }
//! ```
//! A [`InputEvent`] with a type of [`EventType::KEY`] a code of [`KeyCode::KEY_ENTER`] and a
//! value of 1 is emitted when the Enter key is pressed.
//!
//! All events (even single events) are sent in batches followed by a synchronization event:
//! `EV_SYN / SYN_REPORT / 0`.
//! Events are grouped into batches based on if they are related and occur simultaneously,
//! for example movement of a mouse triggers a movement event for the `X` and `Y` axes
//! separately in a batch of 2 events.
//!
//! The evdev crate exposes functions to query the current state of a device from the kernel, as
//! well as a function that can be called continuously to provide an iterator over update events
//! as they arrive.
//!
//! ## Matching Events
//!
//! When reading from an input Device it is often useful to check which type/code or value
//! the event has. This library provides the [`EventSummary`] enum which can be used to
//! match specific events. Calling [`InputEvent::destructure`] will return that enum.
//!
//! ```no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use evdev::*;
//! let mut device = Device::open("/dev/input/event0")?;
//! loop {
//!     for event in device.fetch_events().unwrap(){
//!         match event.destructure(){
//!             EventSummary::Key(ev, KeyCode::KEY_A, 1) => {
//!                 println!("Key 'a' was pressed, got event: {:?}", ev);
//!             },
//!             EventSummary::Key(_, key_type, 0) => {
//!                 println!("Key {:?} was released", key_type);
//!             },
//!             EventSummary::AbsoluteAxis(_, axis, value) => {
//!                 println!("The Axis {:?} was moved to {}", axis, value);
//!             },
//!             _ => println!("got a different event!")
//!         }
//!     }
//! }
//! # unreachable!()
//! # }
//! ```
//!
//! # Synchronizing versus Raw modes
//!
//! This library can be used in either Raw or Synchronizing modes, which correspond roughly to
//! evdev's `LIBEVDEV_READ_FLAG_NORMAL` and `LIBEVDEV_READ_FLAG_SYNC` modes, respectively.
//! In both modes, calling `fetch_events` and driving the resulting iterator to completion
//! will provide a stream of real-time events from the underlying kernel device state.
//! As the state changes, the kernel will write events into a ring buffer. If the buffer becomes full, the
//! kernel will *drop* events from the ring buffer and leave an event telling userspace that it
//! did so. At this point, if the application were using the events it received to update its
//! internal idea of what state the hardware device is in, it will be wrong: it is missing some
//! events.
//!
//! In synchronous mode, this library tries to ease that pain by removing the corrupted events
//! and injecting fake events as if the device had updated normally. Note that this is best-effort;
//! events can never be recovered once lost. This synchronization comes at a performance cost: each
//! set of input events read from the kernel in turn updates an internal state buffer, and events
//! must be internally held back until the end of each frame. If this latency is unacceptable or
//! for any reason you want to see every event directly, a raw stream reader is also provided.
//!
//! As an example of how synchronization behaves, if a switch is toggled twice there will be two switch events
//! in the buffer. However, if the kernel needs to drop events, when the device goes to synchronize
//! state with the kernel only one (or zero, if the switch is in the same state as it was before
//! the sync) switch events will be visible in the stream.
//!
//! This cache can also be queried. For example, the [`DeviceState::led_vals`] method will tell you which
//! LEDs are currently lit on the device. As calling code consumes each iterator, this state will be
//! updated, and it will be fully re-synchronized with the kernel if the stream drops any events.
//!
//! It is recommended that you dedicate a thread to processing input events, or use epoll or an
//! async runtime with the fd returned by `<Device as AsRawFd>::as_raw_fd` to process events when
//! they are ready.
//!
//! For demonstrations of how to use this library in blocking, nonblocking, and async (tokio) modes,
//! please reference the "examples" directory.

// should really be cfg(target_os = "linux") and maybe also android?
#![cfg(unix)]
// Flag items' docs' with their required feature flags, but only on docsrs so
// that local docs can still be built on stable toolchains.
// As of the time of writing, the stabilization plan is such that:
// - Once stabilized, this attribute should be replaced with #![doc(auto_cfg)]
// - Then in edition 2024, doc(auto_cfg) will become the default and the
//   attribute can be removed entirely
// (see https://github.com/rust-lang/rust/pull/100883#issuecomment-1264470491)
#![cfg_attr(docsrs, feature(doc_auto_cfg))]

// has to be first for its macro
#[macro_use]
mod attribute_set;

mod compat;
mod constants;
mod device_state;
pub mod event_variants;
mod ff;
mod inputid;
pub mod raw_stream;
mod scancodes;
mod sync_stream;
mod sys;
#[cfg(test)]
mod tests;
pub mod uinput;

use crate::compat::{input_absinfo, input_event, uinput_abs_setup};
use std::fmt::{self, Display};
use std::io;
use std::os::fd::{AsFd, AsRawFd, OwnedFd};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

pub use attribute_set::{AttributeSet, AttributeSetRef, EvdevEnum};
pub use constants::*;
pub use device_state::DeviceState;
pub use event_variants::*;
pub use ff::*;
pub use inputid::*;
pub use scancodes::*;
pub use sync_stream::*;

macro_rules! common_trait_impls {
    ($raw:ty, $wrapper:ty) => {
        impl From<$raw> for $wrapper {
            fn from(raw: $raw) -> Self {
                Self(raw)
            }
        }

        impl From<$wrapper> for $raw {
            fn from(wrapper: $wrapper) -> Self {
                wrapper.0
            }
        }

        impl AsRef<$raw> for $wrapper {
            fn as_ref(&self) -> &$raw {
                &self.0
            }
        }
    };
}

const EVENT_BATCH_SIZE: usize = 32;

/// A convenience mapping for matching a [`InputEvent`] while simultaniously checking its kind `(type, code)`
/// and capturing the value
///
/// Note This enum can not enforce that `InputEvent.code() == ` enum variant(code).
/// It is suggested to not construct this enum and instead use [`InputEvent::destructure`] to obtain instances.
#[derive(Debug)]
pub enum EventSummary {
    Synchronization(SynchronizationEvent, SynchronizationCode, i32),
    Key(KeyEvent, KeyCode, i32),
    RelativeAxis(RelativeAxisEvent, RelativeAxisCode, i32),
    AbsoluteAxis(AbsoluteAxisEvent, AbsoluteAxisCode, i32),
    Misc(MiscEvent, MiscCode, i32),
    Switch(SwitchEvent, SwitchCode, i32),
    Led(LedEvent, LedCode, i32),
    Sound(SoundEvent, SoundCode, i32),
    Repeat(RepeatEvent, RepeatCode, i32),
    ForceFeedback(FFEvent, FFEffectCode, i32),
    Power(PowerEvent, PowerCode, i32),
    ForceFeedbackStatus(FFStatusEvent, FFStatusCode, i32),
    UInput(UInputEvent, UInputCode, i32),
    Other(OtherEvent, OtherCode, i32),
}

impl From<InputEvent> for EventSummary {
    fn from(value: InputEvent) -> Self {
        match value.event_type() {
            EventType::SYNCHRONIZATION => SynchronizationEvent::from_event(value).into(),
            EventType::KEY => KeyEvent::from_event(value).into(),
            EventType::RELATIVE => RelativeAxisEvent::from_event(value).into(),
            EventType::ABSOLUTE => AbsoluteAxisEvent::from_event(value).into(),
            EventType::MISC => MiscEvent::from_event(value).into(),
            EventType::SWITCH => SwitchEvent::from_event(value).into(),
            EventType::LED => LedEvent::from_event(value).into(),
            EventType::SOUND => SoundEvent::from_event(value).into(),
            EventType::REPEAT => RepeatEvent::from_event(value).into(),
            EventType::FORCEFEEDBACK => FFEvent::from_event(value).into(),
            EventType::POWER => PowerEvent::from_event(value).into(),
            EventType::FORCEFEEDBACKSTATUS => FFStatusEvent::from_event(value).into(),
            EventType::UINPUT => UInputEvent::from_event(value).into(),
            _ => OtherEvent(value).into(),
        }
    }
}

/// A wrapped `input_absinfo` returned by EVIOCGABS and used with uinput to set up absolute
/// axes
///
/// `input_absinfo` is a struct containing six fields:
/// - `value: s32`
/// - `minimum: s32`
/// - `maximum: s32`
/// - `fuzz: s32`
/// - `flat: s32`
/// - `resolution: s32`
///
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct AbsInfo(input_absinfo);

impl AbsInfo {
    #[inline]
    pub fn value(&self) -> i32 {
        self.0.value
    }
    #[inline]
    pub fn minimum(&self) -> i32 {
        self.0.minimum
    }
    #[inline]
    pub fn maximum(&self) -> i32 {
        self.0.maximum
    }
    #[inline]
    pub fn fuzz(&self) -> i32 {
        self.0.fuzz
    }
    #[inline]
    pub fn flat(&self) -> i32 {
        self.0.flat
    }
    #[inline]
    pub fn resolution(&self) -> i32 {
        self.0.resolution
    }

    /// Creates a new AbsInfo, particurarily useful for uinput
    pub fn new(
        value: i32,
        minimum: i32,
        maximum: i32,
        fuzz: i32,
        flat: i32,
        resolution: i32,
    ) -> Self {
        AbsInfo(input_absinfo {
            value,
            minimum,
            maximum,
            fuzz,
            flat,
            resolution,
        })
    }
}

common_trait_impls!(input_absinfo, AbsInfo);

/// A wrapped `uinput_abs_setup`, used to set up analogue axes with uinput
///
/// `uinput_abs_setup` is a struct containing two fields:
/// - `code: u16`
/// - `absinfo: input_absinfo`
#[derive(Debug, Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct UinputAbsSetup(uinput_abs_setup);

impl UinputAbsSetup {
    #[inline]
    pub fn code(&self) -> u16 {
        self.0.code
    }
    #[inline]
    pub fn absinfo(&self) -> AbsInfo {
        AbsInfo(self.0.absinfo)
    }
    /// Creates new UinputAbsSetup
    pub fn new(code: AbsoluteAxisCode, absinfo: AbsInfo) -> Self {
        UinputAbsSetup(uinput_abs_setup {
            code: code.0,
            absinfo: absinfo.0,
        })
    }
}

common_trait_impls!(uinput_abs_setup, UinputAbsSetup);

/// A wrapped `input_event` returned by the input device via the kernel.
///
/// `input_event` is a struct containing four fields:
/// - `time: timeval`
/// - `type_: u16`
/// - `code: u16`
/// - `value: s32`
///
/// The meaning of the "code" and "value" fields will depend on the underlying type of event.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct InputEvent(input_event);
common_trait_impls!(input_event, InputEvent);

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

    /// Returns the raw "value" field directly from input_event.
    ///
    /// For keys and switches the values 0 and 1 map to pressed and not pressed respectively.
    /// For axes, the values depend on the hardware and driver implementation.
    #[inline]
    pub fn value(&self) -> i32 {
        self.0.value
    }

    /// A convenience function to destructure the InputEvent into a [`EventSummary`].
    ///
    /// # Example
    /// ```
    /// use evdev::*;
    /// let event =  InputEvent::new(1, KeyCode::KEY_A.0, 1);
    /// match event.destructure() {
    ///     EventSummary::Key(KeyEvent, KeyCode::KEY_A, 1) => (),
    ///     _=> panic!(),
    /// }
    /// ```
    pub fn destructure(self) -> EventSummary {
        self.into()
    }

    /// Create a new InputEvent. Only really useful for emitting events on virtual devices.
    pub fn new(type_: u16, code: u16, value: i32) -> Self {
        let raw = input_event {
            time: libc::timeval {
                tv_sec: 0,
                tv_usec: 0,
            },
            type_,
            code,
            value,
        };
        Self(raw)
    }

    /// Create a new InputEvent with the time field set to "now" on the system clock.
    ///
    /// Note that this isn't usually necessary simply for emitting events on a virtual device, as
    /// even though [`InputEvent::new`] creates an `input_event` with the time field as zero,
    /// the kernel will update `input_event.time` when it emits the events to any programs reading
    /// the event "file".
    pub fn new_now(type_: u16, code: u16, value: i32) -> Self {
        let raw = input_event {
            time: systime_to_timeval(&SystemTime::now()),
            type_,
            code,
            value,
        };
        Self(raw)
    }
}

impl fmt::Debug for InputEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let summary = self.destructure();
        let code: &dyn fmt::Debug = match &summary {
            EventSummary::Synchronization(_, code, _) => code,
            EventSummary::Key(_, code, _) => code,
            EventSummary::RelativeAxis(_, code, _) => code,
            EventSummary::AbsoluteAxis(_, code, _) => code,
            EventSummary::Misc(_, code, _) => code,
            EventSummary::Switch(_, code, _) => code,
            EventSummary::Led(_, code, _) => code,
            EventSummary::Sound(_, code, _) => code,
            EventSummary::Repeat(_, code, _) => code,
            EventSummary::ForceFeedback(_, code, _) => code,
            EventSummary::Power(_, code, _) => code,
            EventSummary::ForceFeedbackStatus(_, code, _) => code,
            EventSummary::UInput(_, code, _) => code,
            EventSummary::Other(_, code, _) => &code.1,
        };
        f.debug_struct("InputEvent")
            .field("time", &self.timestamp())
            .field("type", &self.event_type())
            .field("code", code)
            .field("value", &self.value())
            .finish()
    }
}

/// Crawls `/dev/input` for evdev devices.
///
/// Will not bubble up any errors in opening devices or traversing the directory. Instead returns
/// an empty iterator or omits the devices that could not be opened.
pub fn enumerate() -> EnumerateDevices {
    EnumerateDevices {
        inner: raw_stream::enumerate(),
    }
}

/// An iterator over currently connected evdev devices.
pub struct EnumerateDevices {
    inner: raw_stream::EnumerateDevices,
}
impl Iterator for EnumerateDevices {
    type Item = (PathBuf, Device);
    fn next(&mut self) -> Option<(PathBuf, Device)> {
        self.inner
            .next()
            .map(|(pb, dev)| (pb, Device::from_raw_device(dev)))
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
    let dur = Duration::new(tv.tv_sec as u64, tv.tv_usec as u32 * 1000);
    if tv.tv_sec >= 0 {
        SystemTime::UNIX_EPOCH + dur
    } else {
        SystemTime::UNIX_EPOCH - dur
    }
}

/// SAFETY: T must not have any padding or otherwise uninitialized bytes inside of it
pub(crate) unsafe fn cast_to_bytes<T: ?Sized>(mem: &T) -> &[u8] {
    std::slice::from_raw_parts(mem as *const T as *const u8, std::mem::size_of_val(mem))
}

/// An error type for the `FromStr` implementation for enum-like types in this crate.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EnumParseError(());

impl Display for EnumParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "failed to parse Key from string")
    }
}

impl std::error::Error for EnumParseError {}

fn fd_write_all(fd: std::os::fd::BorrowedFd<'_>, mut data: &[u8]) -> nix::Result<()> {
    loop {
        match nix::unistd::write(fd, data) {
            Ok(0) => return Ok(()),
            Ok(n) => data = &data[n..],
            Err(nix::Error::EINTR) => {}
            Err(e) => return Err(e),
        }
    }
}

fn write_events(fd: std::os::fd::BorrowedFd<'_>, events: &[InputEvent]) -> nix::Result<()> {
    let bytes = unsafe { cast_to_bytes(events) };
    fd_write_all(fd, bytes)
}

/// Represents a force feedback effect that has been successfully uploaded to the device for
/// playback.
#[derive(Debug)]
pub struct FFEffect {
    fd: OwnedFd,
    id: u16,
}

impl FFEffect {
    /// Returns the effect ID.
    pub fn id(&self) -> u16 {
        self.id
    }

    /// Plays the force feedback effect with the `count` argument specifying how often the effect
    /// should be played.
    pub fn play(&mut self, count: i32) -> io::Result<()> {
        let events = [*FFEvent::new(FFEffectCode(self.id), count)];
        crate::write_events(self.fd.as_fd(), &events)?;

        Ok(())
    }

    /// Stops playback of the force feedback effect.
    pub fn stop(&mut self) -> io::Result<()> {
        let events = [*FFEvent::new(FFEffectCode(self.id), 0)];
        crate::write_events(self.fd.as_fd(), &events)?;

        Ok(())
    }

    /// Updates the force feedback effect.
    pub fn update(&mut self, data: FFEffectData) -> io::Result<()> {
        let mut effect: sys::ff_effect = data.into();
        effect.id = self.id as i16;

        unsafe { sys::eviocsff(self.fd.as_raw_fd(), &effect)? };

        Ok(())
    }
}

impl Drop for FFEffect {
    fn drop(&mut self) {
        let _ = unsafe { sys::eviocrmff(self.fd.as_raw_fd(), self.id as _) };
    }
}

/// Auto-repeat settings for a device.
#[derive(Debug, Clone)]
#[repr(C)]
pub struct AutoRepeat {
    /// The duration, in milliseconds, that a key needs to be held down before
    /// it begins to auto-repeat.
    pub delay: u32,
    /// The duration, in milliseconds, between auto-repetitions of a held-down key.
    pub period: u32,
}
