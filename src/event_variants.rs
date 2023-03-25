use std::fmt;
use std::time::SystemTime;

use crate::compat::input_event;
use crate::constants::{
    AbsoluteAxisType, FFStatusType, LedType, MiscType, OtherType, PowerType, RelativeAxisType,
    RepeatType, SoundType, SwitchType, SynchronizationType, UInputType,
};
use crate::scancodes::KeyType;
use crate::{systime_to_timeval, timeval_to_systime, EventData, EventType, FFEffectType};

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
/// A bookkeeping event. Usually not important to applications.
/// [`EventType::SYNCHRONIZATION`]
pub struct SynchronizationEvent(input_event);

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
/// [`EventType::KEY`]
pub struct KeyEvent(input_event);

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
/// [`EventType::RELATIVE`]
pub struct RelativeAxisEvent(input_event);

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
/// [`EventType::ABSOLUTE`]
pub struct AbsoluteAxisEvent(input_event);

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
/// [`EventType::MISC`]
pub struct MiscEvent(input_event);

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
/// [`EventType::SWITCH`]
pub struct SwitchEvent(input_event);

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
/// [`EventType::LED`]
pub struct LedEvent(input_event);
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
/// [`EventType::SOUND`]
pub struct SoundEvent(input_event);

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
/// [`EventType::REPEAT`]
pub struct RepeatEvent(input_event);

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
/// [`EventType::FORCEFEEDBACK`]
pub struct FFEvent(input_event);

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
/// [`EventType::POWER`]
pub struct PowerEvent(input_event);

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
/// [`EventType::FORCEFEEDBACKSTATUS`]
pub struct FFStatusEvent(input_event);

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
/// [`EventType::UINPUT`]
pub struct UInputEvent(input_event);

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(transparent)]
/// No clue, but technically possible.
pub struct OtherEvent(pub(crate) input_event);

macro_rules! input_event_newtype {
    ($name:ty) => {
        impl EventData for $name {
            #[inline]
            fn timestamp(&self) -> SystemTime {
                timeval_to_systime(&self.0.time)
            }
            #[inline]
            fn event_type(&self) -> u16 {
                self.0.type_
            }
            #[inline]
            fn code(&self) -> u16 {
                self.0.code
            }
            #[inline]
            fn value(&self) -> i32 {
                self.0.value
            }
        }
        impl AsRef<input_event> for $name {
            fn as_ref(&self) -> &input_event {
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
                    $evdev_type => Self(raw),
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
