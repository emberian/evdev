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
//! Devices emit events, represented by the [`EvdevEvent`] trait. Each device supports a few different
//! kinds of events, specified by the [`EventType`] struct and the [`Device::supported_events()`]
//! method. The [`InputEvent`] enum implements the `EvdevEvent` trait and has a variant for each
//! `EventType`. Most event types also have a "subtype", e.g. a `KEY` event with a `KEY_ENTER` code.
//! This type+subtype combo is represented by [`InputEventKind`]/[`InputEvent::kind()`]. The individual
//! subtypes of a type that a device supports can be retrieved through the `Device::supported_*()`
//! methods, e.g. [`Device::supported_keys()`]:
//!
//! ```no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use evdev::{Device, KeyType};
//! let device = Device::open("/dev/input/event0")?;
//! // check if the device has an ENTER key
//! if device.supported_keys().map_or(false, |keys| keys.contains(KeyType::KEY_ENTER)) {
//!     println!("are you prepared to ENTER the world of evdev?");
//! } else {
//!     println!(":(");
//! }
//! # Ok(())
//! # }
//! ```
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
//! # Matching Events
//!
//! When reading from an input Device it is often useful to check which type/subtype or value
//! the event has. This library provides the [`InputEventMatcher`] enum which can be used to
//! match specific events. Calling [`InputEvent::matcher`] will return that enum.
//!
//! ```no_run
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! use evdev::*;
//! let mut device = Device::open("/dev/input/event0")?;
//! loop {
//!     for event in device.fetch_events().unwrap(){
//!         match event.matcher(){
//!             InputEventMatcher::Key(ev, KeyType::KEY_A, 1) => {
//!                 println!("Key 'a' was pressed, got event: {:?}", ev);
//!             },
//!             InputEventMatcher::Key(_, key_type, 0) => {
//!                 println!("Key {:?} was released", key_type);
//!             },
//!             InputEventMatcher::AbsoluteAxis(_, axis, value) => {
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
mod error;
mod event_variants;
mod ff;
mod inputid;
pub mod raw_stream;
mod scancodes;
mod sync_stream;
mod sys;
pub mod uinput;

#[cfg(feature = "serde")]
use serde_1::{Deserialize, Serialize};

use crate::compat::{input_absinfo, input_event, uinput_abs_setup};
use std::fmt;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

pub use attribute_set::{AttributeSet, AttributeSetRef, EvdevEnum};
pub use constants::*;
pub use device_state::DeviceState;
pub use error::Error;
pub use event_variants::*;
pub use ff::*;
pub use inputid::*;
pub use raw_stream::{AutoRepeat, FFEffect};
pub use scancodes::*;
pub use sync_stream::*;

const EVENT_BATCH_SIZE: usize = 32;

/// A convenience mapping from an event `(type, code)` to an enumeration.
///
/// Note that this does not capture the event or its value, just the type and code.
/// Use [`InputEventMatcher`] for that.
#[derive(Debug, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(crate = "serde_1"))]
pub enum InputEventKind {
    Synchronization(SynchronizationType),
    Key(KeyType),
    RelativeAxis(RelativeAxisType),
    AbsoluteAxis(AbsoluteAxisType),
    Misc(MiscType),
    Switch(SwitchType),
    Led(LedType),
    Sound(SoundType),
    Repeat(RepeatType),
    ForceFeedback(FFEffectType),
    Power(PowerType),
    ForceFeedbackStatus(FFStatusType),
    UInput(UInputType),
    Other(OtherType),
}

/// A convenience mapping for matching a [`InputEvent`] while simultaniously checking its kind `(type, code)`
/// and capturing the value
///
/// Note This enum can not enforce that `InputEvent.code() == ` enum variant(code).
/// It is suggested to not construct this enum and instead use `InputEvent.matcher()` to obtain instances.
pub enum InputEventMatcher {
    Synchronization(SynchronizationEvent, SynchronizationType, i32),
    Key(KeyEvent, KeyType, i32),
    RelativeAxis(RelativeAxisEvent, RelativeAxisType, i32),
    AbsoluteAxis(AbsoluteAxisEvent, AbsoluteAxisType, i32),
    Misc(MiscEvent, MiscType, i32),
    Switch(SwitchEvent, SwitchType, i32),
    Led(LedEvent, LedType, i32),
    Sound(SoundEvent, SoundType, i32),
    Repeat(RepeatEvent, RepeatType, i32),
    ForceFeedback(FFEvent, FFEffectType, i32),
    Power(PowerEvent, PowerType, i32),
    ForceFeedbackStatus(FFStatusEvent, FFStatusType, i32),
    UInput(UInputEvent, UInputType, i32),
    Other(OtherEvent, OtherType, i32),
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
#[derive(Copy, Clone)]
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

/// A wrapped `uinput_abs_setup`, used to set up analogue axes with uinput
///
/// `uinput_abs_setup` is a struct containing two fields:
/// - `code: u16`
/// - `absinfo: input_absinfo`
#[derive(Copy, Clone)]
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
    pub fn new(code: AbsoluteAxisType, absinfo: AbsInfo) -> Self {
        UinputAbsSetup(uinput_abs_setup {
            code: code.0,
            absinfo: absinfo.0,
        })
    }
}

/// The common trait for all [`InputEvent`] variants and the `InputEvent` itself.
/// Anything that implements this can be sent to a [`Device`] or [`uinput::VirtualDevice`]
pub trait EvdevEvent: AsRef<input_event> {
    /// Returns the timestamp associated with the event.
    fn timestamp(&self) -> SystemTime;
    /// Returns the "type" field directly from input_event.
    fn event_type(&self) -> u16;
    /// Returns the raw "code" field directly from input_event.
    fn code(&self) -> u16;
    /// Returns the raw "value" field directly from input_event.
    ///
    /// For keys and switches the values 0 and 1 map to pressed and not pressed respectively.
    /// For axes, the values depend on the hardware and driver implementation.
    fn value(&self) -> i32;
}

/// A wrapped `input_event` returned by the input device via the kernel.
///
/// `input_event` is a struct containing four fields:
/// - `time: timeval`
/// - `type_: u16`
/// - `code: u16`
/// - `value: s32`
///
/// The meaning of the "code" and "value" fields will depend on the underlying type of event.
#[derive(Copy, Clone)]
pub enum InputEvent {
    /// [`SynchronizationEvent`]
    Synchronization(SynchronizationEvent),
    /// [`KeyEvent`]
    Key(KeyEvent),
    /// [`RelativeAxisEvent`]
    RelativeAxis(RelativeAxisEvent),
    /// [`AbsoluteAxisEvent`]
    AbsoluteAxis(AbsoluteAxisEvent),
    /// [`MiscEvent`]
    Misc(MiscEvent),
    /// [`SwitchEvent`]
    Switch(SwitchEvent),
    /// [`LedEvent`]
    Led(LedEvent),
    /// [`SoundEvent`]
    Sound(SoundEvent),
    /// [`RepeatEvent`]
    Repeat(RepeatEvent),
    /// [`FFEvent`]
    ForceFeedback(FFEvent),
    /// [`PowerEvent`]
    Power(PowerEvent),
    /// [`FFStatusEvent`]
    ForceFeedbackStatus(FFStatusEvent),
    /// [`UInputEvent`]
    UInput(UInputEvent),
    /// [`OtherEvent`]
    Other(OtherEvent),
}

macro_rules! call_at_each_variant {
    ($self:ident, $method:ident $(, $args:expr)*) => {
        match $self {
            InputEvent::Synchronization(ev) => ev.$method($($args),*),
            InputEvent::Key(ev) => ev.$method($($args),*),
            InputEvent::RelativeAxis(ev) => ev.$method($($args),*),
            InputEvent::AbsoluteAxis(ev) => ev.$method($($args),*),
            InputEvent::Misc(ev) => ev.$method($($args),*),
            InputEvent::Switch(ev) => ev.$method($($args),*),
            InputEvent::Led(ev) => ev.$method($($args),*),
            InputEvent::Sound(ev) => ev.$method($($args),*),
            InputEvent::Repeat(ev) => ev.$method($($args),*),
            InputEvent::ForceFeedback(ev) => ev.$method($($args),*),
            InputEvent::Power(ev) => ev.$method($($args),*),
            InputEvent::ForceFeedbackStatus(ev) => ev.$method($($args),*),
            InputEvent::UInput(ev) => ev.$method($($args),*),
            InputEvent::Other(ev) => ev.$method($($args),*),
        }
    };
}

impl InputEvent {
    /// A convenience function to return the `self.code()` wrapped in a
    /// certain newtype corresponding to the `InputEvent` variant.
    ///
    /// This is useful if you want to match events by specific key codes or axes.
    /// Note that this does not capture the event value, just the type and code.
    ///
    /// # Example
    /// ```
    /// use evdev::*;
    /// let event =  InputEvent::new(1, KeyType::KEY_A.0, 1);
    /// match event.kind() {
    ///     InputEventKind::Key(KeyType::KEY_A) =>
    ///         println!("Matched KeyEvent of type {:?}", KeyType::KEY_A),
    ///     _=> panic!(),
    /// }
    /// ```
    #[inline]
    pub fn kind(&self) -> InputEventKind {
        match self {
            InputEvent::Synchronization(ev) => InputEventKind::Synchronization(ev.kind()),
            InputEvent::Key(ev) => InputEventKind::Key(ev.kind()),
            InputEvent::RelativeAxis(ev) => InputEventKind::RelativeAxis(ev.kind()),
            InputEvent::AbsoluteAxis(ev) => InputEventKind::AbsoluteAxis(ev.kind()),
            InputEvent::Misc(ev) => InputEventKind::Misc(ev.kind()),
            InputEvent::Switch(ev) => InputEventKind::Switch(ev.kind()),
            InputEvent::Led(ev) => InputEventKind::Led(ev.kind()),
            InputEvent::Sound(ev) => InputEventKind::Sound(ev.kind()),
            InputEvent::Repeat(ev) => InputEventKind::Repeat(ev.kind()),
            InputEvent::ForceFeedback(ev) => InputEventKind::ForceFeedback(ev.kind()),
            InputEvent::Power(ev) => InputEventKind::Power(ev.kind()),
            InputEvent::ForceFeedbackStatus(ev) => InputEventKind::ForceFeedbackStatus(ev.kind()),
            InputEvent::UInput(ev) => InputEventKind::UInput(ev.kind()),
            InputEvent::Other(ev) => InputEventKind::Other(ev.kind()),
        }
    }

    /// A convenience function to return the `InputEvent` its `kind()` and `value()` wrapped in a
    /// certain newtype corresponding to the `InputEvent` variant.
    ///
    /// # Example
    /// ```
    /// use evdev::*;
    /// let event =  InputEvent::new(1, KeyType::KEY_A.0, 1);
    /// match event.matcher() {
    ///     InputEventMatcher::Key(KeyEvent, KeyType::KEY_A, 1) => (),
    ///     _=> panic!(),
    /// }
    /// ```
    #[inline]
    pub fn matcher(self) -> InputEventMatcher {
        match self {
            InputEvent::Synchronization(ev) => {
                InputEventMatcher::Synchronization(ev, ev.kind(), ev.value())
            }
            InputEvent::Key(ev) => InputEventMatcher::Key(ev, ev.kind(), ev.value()),
            InputEvent::RelativeAxis(ev) => {
                InputEventMatcher::RelativeAxis(ev, ev.kind(), ev.value())
            }
            InputEvent::AbsoluteAxis(ev) => {
                InputEventMatcher::AbsoluteAxis(ev, ev.kind(), ev.value())
            }
            InputEvent::Misc(ev) => InputEventMatcher::Misc(ev, ev.kind(), ev.value()),
            InputEvent::Switch(ev) => InputEventMatcher::Switch(ev, ev.kind(), ev.value()),
            InputEvent::Led(ev) => InputEventMatcher::Led(ev, ev.kind(), ev.value()),
            InputEvent::Sound(ev) => InputEventMatcher::Sound(ev, ev.kind(), ev.value()),
            InputEvent::Repeat(ev) => InputEventMatcher::Repeat(ev, ev.kind(), ev.value()),
            InputEvent::ForceFeedback(ev) => {
                InputEventMatcher::ForceFeedback(ev, ev.kind(), ev.value())
            }
            InputEvent::Power(ev) => InputEventMatcher::Power(ev, ev.kind(), ev.value()),
            InputEvent::ForceFeedbackStatus(ev) => {
                InputEventMatcher::ForceFeedbackStatus(ev, ev.kind(), ev.value())
            }
            InputEvent::UInput(ev) => InputEventMatcher::UInput(ev, ev.kind(), ev.value()),
            InputEvent::Other(ev) => InputEventMatcher::Other(ev, ev.kind(), ev.value()),
        }
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
        Self::from(raw)
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
        Self::from(raw)
    }
}

impl From<input_event> for InputEvent {
    fn from(raw: input_event) -> Self {
        match EventType(raw.type_) {
            EventType::SYNCHRONIZATION => {
                InputEvent::Synchronization(SynchronizationEvent::from(raw))
            }
            EventType::KEY => InputEvent::Key(KeyEvent::from(raw)),
            EventType::RELATIVE => InputEvent::RelativeAxis(RelativeAxisEvent::from(raw)),
            EventType::ABSOLUTE => InputEvent::AbsoluteAxis(AbsoluteAxisEvent::from(raw)),
            EventType::MISC => InputEvent::Misc(MiscEvent::from(raw)),
            EventType::SWITCH => InputEvent::Switch(SwitchEvent::from(raw)),
            EventType::LED => InputEvent::Led(LedEvent::from(raw)),
            EventType::SOUND => InputEvent::Sound(SoundEvent::from(raw)),
            EventType::FORCEFEEDBACK => InputEvent::ForceFeedback(FFEvent::from(raw)),
            EventType::FORCEFEEDBACKSTATUS => {
                InputEvent::ForceFeedbackStatus(FFStatusEvent::from(raw))
            }
            EventType::UINPUT => InputEvent::UInput(UInputEvent::from(raw)),
            _ => InputEvent::Other(OtherEvent(raw)),
        }
    }
}

macro_rules! impl_from_type {
    ($type:ty, $variant:path) => {
        impl From<$type> for InputEvent {
            fn from(value: $type) -> Self {
                $variant(value)
            }
        }
    };
}
impl_from_type!(SynchronizationEvent, InputEvent::Synchronization);
impl_from_type!(KeyEvent, InputEvent::Key);
impl_from_type!(RelativeAxisEvent, InputEvent::RelativeAxis);
impl_from_type!(AbsoluteAxisEvent, InputEvent::AbsoluteAxis);
impl_from_type!(MiscEvent, InputEvent::Misc);
impl_from_type!(SwitchEvent, InputEvent::Switch);
impl_from_type!(LedEvent, InputEvent::Led);
impl_from_type!(SoundEvent, InputEvent::Sound);
impl_from_type!(FFEvent, InputEvent::ForceFeedback);
impl_from_type!(FFStatusEvent, InputEvent::ForceFeedbackStatus);
impl_from_type!(UInputEvent, InputEvent::UInput);
impl_from_type!(OtherEvent, InputEvent::Other);

impl EvdevEvent for InputEvent {
    fn code(&self) -> u16 {
        call_at_each_variant!(self, code)
    }
    fn event_type(&self) -> u16 {
        call_at_each_variant!(self, event_type)
    }
    fn timestamp(&self) -> SystemTime {
        call_at_each_variant!(self, timestamp)
    }
    fn value(&self) -> i32 {
        call_at_each_variant!(self, value)
    }
}

impl AsRef<input_event> for InputEvent {
    fn as_ref(&self) -> &input_event {
        call_at_each_variant!(self, as_ref)
    }
}

impl fmt::Debug for InputEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        call_at_each_variant!(self, fmt, f)
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumParseError(());
