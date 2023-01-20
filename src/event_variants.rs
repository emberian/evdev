use std::time::{SystemTime, Duration};

use crate::compat::input_event;
use crate::constants::EventType;

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
pub struct ForceFeedbackEvent(input_event);
#[derive(Copy, Clone)]
#[repr(transparent)]
/// I think this is unused?
pub struct  PowerEvent(input_event);
#[derive(Copy, Clone)]
#[repr(transparent)]
/// A force feedback effect's state changed.
pub struct ForceFeedbackStatusEvent(input_event);
#[derive(Copy, Clone)]
#[repr(transparent)]
/// An event originating from uinput.
pub struct UInputEvent(input_event);
#[derive(Copy, Clone)]
#[repr(transparent)]
/// No clue, but technically possible.
pub struct OtherEvent(input_event);

pub trait UnixEvent {
    /// Returns the timestamp associated with the event.
    fn timestamp(&self) -> SystemTime;
    /// Returns the type of event this describes, e.g. Key, Switch, etc.
    fn event_type(&self) -> EventType;
    /// Returns the raw "code" field directly from input_event.
    fn code(&self) -> u16;
    /// Returns the raw "value" field directly from input_event.
    ///
    /// For keys and switches the values 0 and 1 map to pressed and not pressed respectively.
    /// For axes, the values depend on the hardware and driver implementation.
    fn value(&self) -> i32;
}

macro_rules! unix_event_trait {
    ($name:ty) => {
        impl UnixEvent for $name {
            #[inline]
            fn timestamp(&self) -> SystemTime{
                timeval_to_systime(&self.0.time)
            }
            #[inline]
            fn event_type(&self) -> EventType {
                EventType(self.0.type_)
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
    };
}
unix_event_trait!(SynchronizationEvent);
unix_event_trait!(KeyEvent);
unix_event_trait!(RelAxisEvent);
unix_event_trait!(AbsAxisEvent);
unix_event_trait!(MiscEvent);
unix_event_trait!(SwitchEvent);
unix_event_trait!(LedEvent);
unix_event_trait!(SoundEvent);
unix_event_trait!(UInputEvent);
unix_event_trait!(OtherEvent);

fn timeval_to_systime(tv: &libc::timeval) -> SystemTime {
    let dur = Duration::new(tv.tv_sec as u64, tv.tv_usec as u32 * 1000);
    if tv.tv_sec >= 0 {
        SystemTime::UNIX_EPOCH + dur
    } else {
        SystemTime::UNIX_EPOCH - dur
    }
}
