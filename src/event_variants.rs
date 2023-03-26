use std::fmt;
use std::time::SystemTime;

use crate::compat::input_event;
use crate::InputEvent;
use crate::constants::{
    AbsoluteAxisType, FFStatusType, LedType, MiscType, OtherType, PowerType, RelativeAxisType,
    RepeatType, SoundType, SwitchType, SynchronizationType, UInputType,
};
use crate::scancodes::KeyType;
use crate::{systime_to_timeval, EventData, EventType, FFEffectType};

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
/// No clue, but technically possible.
pub struct OtherEvent(pub(crate) InputEvent);

macro_rules! input_event_newtype {
    ($name:ty) => {
        impl EventData for $name {
            #[inline]
            fn timestamp(&self) -> SystemTime {
                self.0.timestamp()
            }
            #[inline]
            fn event_type(&self) -> u16 {
                self.0.event_type()
            }
            #[inline]
            fn code(&self) -> u16 {
                self.0.code()
            }
            #[inline]
            fn value(&self) -> i32 {
                self.0.value()
            }
        }
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
        impl From<$name> for InputEvent{
            fn from(event: $name) -> Self { 
                event.0
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
                Self::from(raw)
            }
            pub fn new_now($kind(code): $kind, value: i32) -> Self {
                let raw = input_event {
                    time: systime_to_timeval(&SystemTime::now()),
                    type_: $evdev_type.0,
                    code,
                    value,
                };
                Self::from(raw)
            }

            // must be kept internal
            pub(crate) fn from(raw: input_event) -> Self {
                match EventType(raw.type_) {
                    $evdev_type => Self(InputEvent(raw)),
                    _ => unreachable!(),
                }
            }

            pub fn kind(&self) -> $kind {
                $kind(self.code())
            }
        }
        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                let mut debug = f.debug_struct(stringify!($name));
                debug.field("time", &self.timestamp());
                debug.field("kind", &self.kind());
                debug.field("value", &self.value()).finish()
            }
        }
        input_event_newtype!($name);
    };
}
input_event_newtype!(
    SynchronizationEvent,
    EventType::SYNCHRONIZATION,
    SynchronizationType
);
input_event_newtype!(KeyEvent, EventType::KEY, KeyType);
input_event_newtype!(RelativeAxisEvent, EventType::RELATIVE, RelativeAxisType);
input_event_newtype!(AbsoluteAxisEvent, EventType::ABSOLUTE, AbsoluteAxisType);
input_event_newtype!(MiscEvent, EventType::MISC, MiscType);
input_event_newtype!(SwitchEvent, EventType::SWITCH, SwitchType);
input_event_newtype!(LedEvent, EventType::LED, LedType);
input_event_newtype!(SoundEvent, EventType::SOUND, SoundType);
input_event_newtype!(RepeatEvent, EventType::REPEAT, RepeatType);
input_event_newtype!(FFEvent, EventType::FORCEFEEDBACK, FFEffectType);
input_event_newtype!(PowerEvent, EventType::POWER, PowerType);
input_event_newtype!(FFStatusEvent, EventType::FORCEFEEDBACKSTATUS, FFStatusType);
input_event_newtype!(UInputEvent, EventType::UINPUT, UInputType);
input_event_newtype!(OtherEvent);

impl OtherEvent {
    pub fn kind(&self) -> OtherType {
        OtherType(self.event_type(), self.code())
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
