use std::time::SystemTime;
use std::fmt;

use crate::{EventType, EvdevEvent, timeval_to_systime, systime_to_timeval};
use crate::compat::input_event;
use crate::scancodes::KeyType;
use crate::constants::{
    SynchronizationType, RelAxisType, AbsAxisType, MiscType, SwitchType, 
    LedType, SoundType, RepeatType, PowerType, FFStatusType, UInputType, 
    OtherType, FFType};

#[derive(Copy, Clone)]
#[repr(transparent)]
/// A bookkeeping event. Usually not important to applications.
pub struct SynchronizationEvent(input_event);
#[derive(Copy, Clone)]
#[repr(transparent)]
/// A key changed state. A key, or button, is usually a momentary switch (in the circuit sense). It has two
/// states: down, or up. There are events for when keys are pressed (become down) and
/// released (become up). There are also "key repeats", where multiple events are sent
/// while a key is down.
pub struct KeyEvent(input_event);
#[derive(Copy, Clone)]
#[repr(transparent)]
/// Movement on a relative axis. There is no absolute coordinate frame, just the fact that
/// there was a change of a certain amount of units. Used for things like mouse movement or
/// scroll wheels.
pub struct RelAxisEvent(input_event);
#[derive(Copy, Clone)]
#[repr(transparent)]
/// Movement on an absolute axis. Used for things such as touch events and joysticks.
pub struct AbsAxisEvent(input_event);
#[derive(Copy, Clone)]
#[repr(transparent)]
/// Miscellaneous events that don't fall into other categories. For example, Key presses may
/// send `MSC_SCAN` events before each KEY event
pub struct MiscEvent(input_event);
#[derive(Copy, Clone)]
#[repr(transparent)]
/// Change in a switch value. Switches are boolean conditions and usually correspond to a
/// toggle switch of some kind in hardware.
pub struct SwitchEvent(input_event);
#[derive(Copy, Clone)]
#[repr(transparent)]
/// An LED was toggled.
pub struct LedEvent(input_event);
#[derive(Copy, Clone)]
#[repr(transparent)]
/// A sound was made.
pub struct SoundEvent(input_event);
#[derive(Copy, Clone)]
#[repr(transparent)]
/// There are no events of this type, to my knowledge, but represents metadata about key
/// repeat configuration.
pub struct RepeatEvent(input_event);
#[derive(Copy, Clone)]
#[repr(transparent)]
/// I believe there are no events of this type, but rather this is used to represent that
/// the device can create haptic effects.
pub struct FFEvent(input_event);
#[derive(Copy, Clone)]
#[repr(transparent)]
/// I think this is unused?
pub struct  PowerEvent(input_event);
#[derive(Copy, Clone)]
#[repr(transparent)]
/// A force feedback effect's state changed.
pub struct FFStatusEvent(input_event);
#[derive(Copy, Clone)]
#[repr(transparent)]
/// An event originating from uinput.
pub struct UInputEvent(input_event);
#[derive(Copy, Clone)]
#[repr(transparent)]
/// No clue, but technically possible.
pub struct OtherEvent(pub(crate) input_event);

macro_rules! input_event_newtype {
    ($name:ty) => {
        impl EvdevEvent for $name {
            #[inline]
            fn timestamp(&self) -> SystemTime{
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
            pub fn new(code: u16, value: i32) -> Self{
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
            pub fn new_now(code: u16, value: i32) -> Self{
                let raw = input_event {
                    time: systime_to_timeval(&SystemTime::now()),
                    type_: $evdev_type.0,
                    code,
                    value,
                };
                Self::from(raw)
            }

            // must be kept internal
            pub(crate) fn from(raw: input_event) -> Self{
                match EventType(raw.type_) {
                    $evdev_type => Self(raw),
                    _ => panic!(), // this would be an iternal library error
                }
            }

            pub fn kind(&self) -> $kind{
                $kind(self.code())
            }
        }
        impl fmt::Debug for $name  {
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
input_event_newtype!(SynchronizationEvent, EventType::SYNCHRONIZATION, SynchronizationType);
input_event_newtype!(KeyEvent, EventType::KEY, KeyType);
input_event_newtype!(RelAxisEvent, EventType::RELATIVE, RelAxisType);
input_event_newtype!(AbsAxisEvent, EventType::ABSOLUTE, AbsAxisType);
input_event_newtype!(MiscEvent, EventType::MISC, MiscType);
input_event_newtype!(SwitchEvent, EventType::SWITCH, SwitchType);
input_event_newtype!(LedEvent, EventType::LED, LedType);
input_event_newtype!(SoundEvent, EventType::SOUND, SoundType);
input_event_newtype!(RepeatEvent, EventType::REPEAT, RepeatType);
input_event_newtype!(FFEvent, EventType::FORCEFEEDBACK, FFType);
input_event_newtype!(PowerEvent, EventType::POWER, PowerType);
input_event_newtype!(FFStatusEvent, EventType::FORCEFEEDBACKSTATUS, FFStatusType);
input_event_newtype!(UInputEvent, EventType::UINPUT, UInputType);
input_event_newtype!(OtherEvent);

impl OtherEvent{
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