//! The event_variants module contains new-type wrappers around [`InputEvent`]
//! for each known [`EventType`].
//!
//! These event variants guarantee that the underlying `InputEvent` has the
//! corresponding type. They may also contain additional methods for the
//! specific type and convenient shortcut methods for event creation.
//! An `InputEvent` can be converted to the corresponding event variant with
//! the [`InputEvent::destructure()`] method. Each event variant implements
//! `Into<InputEvent>` and `Deref<Target=InputEvent>` for easy back conversion.

use std::fmt;
use std::ops::Deref;
use std::time::SystemTime;

use crate::compat::input_event;
use crate::constants::{
    AbsoluteAxisCode, FFStatusCode, LedCode, MiscCode, OtherCode, PowerCode, RelativeAxisCode,
    RepeatCode, SoundCode, SwitchCode, SynchronizationCode, UInputCode,
};
use crate::scancodes::KeyCode;
use crate::{systime_to_timeval, EventType, FFEffectCode};
use crate::{EventSummary, InputEvent};

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
/// A bookkeeping event. Usually not important to applications.
/// [`EventType::SYNCHRONIZATION`]
pub struct SynchronizationEvent(InputEvent);

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
/// [`EventType::KEY`]
pub struct KeyEvent(InputEvent);

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
/// [`EventType::RELATIVE`]
pub struct RelativeAxisEvent(InputEvent);

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
/// [`EventType::ABSOLUTE`]
pub struct AbsoluteAxisEvent(InputEvent);

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
/// [`EventType::MISC`]
pub struct MiscEvent(InputEvent);

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
/// [`EventType::SWITCH`]
pub struct SwitchEvent(InputEvent);

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
/// [`EventType::LED`]
pub struct LedEvent(InputEvent);
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
/// [`EventType::SOUND`]
pub struct SoundEvent(InputEvent);

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
/// [`EventType::REPEAT`]
pub struct RepeatEvent(InputEvent);

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
/// [`EventType::FORCEFEEDBACK`]
pub struct FFEvent(InputEvent);

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
/// [`EventType::POWER`]
pub struct PowerEvent(InputEvent);

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
/// [`EventType::FORCEFEEDBACKSTATUS`]
pub struct FFStatusEvent(InputEvent);

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
/// [`EventType::UINPUT`]
pub struct UInputEvent(InputEvent);

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
/// An event not covered by any other variant.
pub struct OtherEvent(pub(crate) InputEvent);

macro_rules! input_event_newtype {
    ($name:ty) => {
        impl AsRef<input_event> for $name {
            fn as_ref(&self) -> &input_event {
                &self.0.as_ref()
            }
        }
        impl AsRef<InputEvent> for $name {
            fn as_ref(&self) -> &InputEvent {
                &self.0
            }
        }
        // never implement the other direction!
        impl From<$name> for InputEvent {
            fn from(event: $name) -> Self {
                event.0
            }
        }
        impl Deref for $name {
            type Target = InputEvent;
            fn deref(&self) -> &InputEvent {
                &self.0
            }
        }
    };
    ($name:ty, $evdev_type:path, $kind:path) => {
        impl $name {
            pub fn new($kind(code): $kind, value: i32) -> Self {
                let raw = input_event {
                    time: libc::timeval {
                        tv_sec: 0,
                        tv_usec: 0,
                    },
                    type_: $evdev_type.0,
                    code,
                    value,
                };
                Self::from_raw(raw)
            }
            pub fn new_now($kind(code): $kind, value: i32) -> Self {
                let raw = input_event {
                    time: systime_to_timeval(&SystemTime::now()),
                    type_: $evdev_type.0,
                    code,
                    value,
                };
                Self::from_raw(raw)
            }
            pub fn destructure(&self) -> ($kind, i32) {
                (self.code(), self.value())
            }
            pub fn code(&self) -> $kind {
                $kind(self.0.code())
            }
            // must be kept internal
            fn from_raw(raw: input_event) -> Self {
                match EventType(raw.type_) {
                    $evdev_type => Self(InputEvent(raw)),
                    _ => unreachable!(),
                }
            }
            // must be kept internal
            pub(crate) fn from_event(event: InputEvent) -> Self {
                match event.event_type() {
                    $evdev_type => Self(event),
                    _ => unreachable!(),
                }
            }
        }
        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                let mut debug = f.debug_struct(stringify!($name));
                debug.field("time", &self.timestamp());
                debug.field("code", &self.code());
                debug.field("value", &self.value()).finish()
            }
        }
        input_event_newtype!($name);
    };
    ($name:ty, $evdev_type:path, $kind:path, $summary:path) => {
        impl From<$name> for EventSummary {
            fn from(event: $name) -> EventSummary {
                let (kind, value) = event.destructure();
                $summary(event, kind, value)
            }
        }

        input_event_newtype!($name, $evdev_type, $kind);
    };
}
input_event_newtype!(
    SynchronizationEvent,
    EventType::SYNCHRONIZATION,
    SynchronizationCode,
    EventSummary::Synchronization
);
input_event_newtype!(KeyEvent, EventType::KEY, KeyCode, EventSummary::Key);
input_event_newtype!(
    RelativeAxisEvent,
    EventType::RELATIVE,
    RelativeAxisCode,
    EventSummary::RelativeAxis
);
input_event_newtype!(
    AbsoluteAxisEvent,
    EventType::ABSOLUTE,
    AbsoluteAxisCode,
    EventSummary::AbsoluteAxis
);
input_event_newtype!(MiscEvent, EventType::MISC, MiscCode, EventSummary::Misc);
input_event_newtype!(
    SwitchEvent,
    EventType::SWITCH,
    SwitchCode,
    EventSummary::Switch
);
input_event_newtype!(LedEvent, EventType::LED, LedCode, EventSummary::Led);
input_event_newtype!(SoundEvent, EventType::SOUND, SoundCode, EventSummary::Sound);
input_event_newtype!(
    RepeatEvent,
    EventType::REPEAT,
    RepeatCode,
    EventSummary::Repeat
);
input_event_newtype!(
    FFEvent,
    EventType::FORCEFEEDBACK,
    FFEffectCode,
    EventSummary::ForceFeedback
);
input_event_newtype!(PowerEvent, EventType::POWER, PowerCode, EventSummary::Power);
input_event_newtype!(
    FFStatusEvent,
    EventType::FORCEFEEDBACKSTATUS,
    FFStatusCode,
    EventSummary::ForceFeedbackStatus
);
input_event_newtype!(
    UInputEvent,
    EventType::UINPUT,
    UInputCode,
    EventSummary::UInput
);
input_event_newtype!(OtherEvent);

impl OtherEvent {
    pub fn kind(&self) -> OtherCode {
        OtherCode(self.event_type().0, self.code())
    }
    pub fn destructure(&self) -> (OtherCode, i32) {
        (self.kind(), self.value())
    }
}
impl From<OtherEvent> for EventSummary {
    fn from(event: OtherEvent) -> Self {
        let (kind, value) = event.destructure();
        EventSummary::Other(event, kind, value)
    }
}
impl fmt::Debug for OtherEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut debug = f.debug_struct("OtherEvent");
        debug.field("time", &self.timestamp());
        debug.field("type", &self.event_type());
        debug.field("code", &self.code());
        debug.field("value", &self.value()).finish()
    }
}
