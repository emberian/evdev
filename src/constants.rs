use crate::compat::{
    ABS_CNT, EV_CNT, FF_CNT, INPUT_PROP_CNT, LED_CNT, MSC_CNT, REL_CNT, SND_CNT, SW_CNT,
};

/// Event types supported by the device.
///
/// Values correspond to [/usr/include/linux/input-event-codes.h](https://github.com/torvalds/linux/blob/master/include/uapi/linux/input-event-codes.h)
///
/// This is implemented as a newtype around the u16 "type" field of `input_event`.
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct EventType(pub u16);

evdev_enum!(
    EventType,
    Array,
    /// A bookkeeping event. Usually not important to applications.
    SYNCHRONIZATION = 0x00,
    /// A key changed state. A key, or button, is usually a momentary switch (in the circuit sense). It has two
    /// states: down, or up. There are events for when keys are pressed (become down) and
    /// released (become up). There are also "key repeats", where multiple events are sent
    /// while a key is down.
    KEY = 0x01,
    /// Movement on a relative axis. There is no absolute coordinate frame, just the fact that
    /// there was a change of a certain amount of units. Used for things like mouse movement or
    /// scroll wheels.
    RELATIVE = 0x02,
    /// Movement on an absolute axis. Used for things such as touch events and joysticks.
    ABSOLUTE = 0x03,
    /// Miscellaneous events that don't fall into other categories. For example, Key presses may
    /// send `MSC_SCAN` events before each KEY event
    MISC = 0x04,
    /// Change in a switch value. Switches are boolean conditions and usually correspond to a
    /// toggle switch of some kind in hardware.
    SWITCH = 0x05,
    /// An LED was toggled.
    LED = 0x11,
    /// A sound was made.
    SOUND = 0x12,
    /// There are no events of this type, to my knowledge, but represents metadata about key
    /// repeat configuration.
    REPEAT = 0x14,
    /// I believe there are no events of this type, but rather this is used to represent that
    /// the device can create haptic effects.
    FORCEFEEDBACK = 0x15,
    /// I think this is unused?
    POWER = 0x16,
    /// A force feedback effect's state changed.
    FORCEFEEDBACKSTATUS = 0x17,
    /// An event originating from uinput.
    UINPUT = 0x0101,
);

impl EventType {
    pub(crate) const COUNT: usize = EV_CNT;
}

/// A "synchronization" message type published by the kernel into the events stream.
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Synchronization(pub u16);

evdev_enum!(
    Synchronization,
    /// Used to mark the end of a single atomic "reading" from the device.
    SYN_REPORT = 0,
    /// Appears to be unused.
    SYN_CONFIG = 1,
    /// "Used to synchronize and separate touch events"
    SYN_MT_REPORT = 2,
    /// Ring buffer filled, events were dropped.
    SYN_DROPPED = 3,
);

/// Device properties.
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct PropType(pub u16);

evdev_enum!(
    PropType,
    Array,
    /// This input device needs a pointer ("cursor") for the user to know its state.
    POINTER = 0x00,
    /// "direct input devices", according to the header.
    DIRECT = 0x01,
    /// "has button(s) under pad", according to the header.
    BUTTONPAD = 0x02,
    /// Touch rectangle only (I think this means that if there are multiple touches, then the
    /// bounding rectangle of all the touches is returned, not each touch).
    SEMI_MT = 0x03,
    /// "softbuttons at top of pad", according to the header.
    TOPBUTTONPAD = 0x04,
    /// Is a pointing stick ("nub" etc, <https://xkcd.com/243/>)
    POINTING_STICK = 0x05,
    /// Has an accelerometer. Probably reports relative events in that case?
    ACCELEROMETER = 0x06,
);

impl PropType {
    pub(crate) const COUNT: usize = INPUT_PROP_CNT;
}

/// A type of relative axis measurement, typically produced by mice.
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct RelativeAxisType(pub u16);

evdev_enum!(
    RelativeAxisType,
    Array,
    REL_X = 0x00,
    REL_Y = 0x01,
    REL_Z = 0x02,
    REL_RX = 0x03,
    REL_RY = 0x04,
    REL_RZ = 0x05,
    REL_HWHEEL = 0x06,
    REL_DIAL = 0x07,
    REL_WHEEL = 0x08,
    REL_MISC = 0x09,
    REL_RESERVED = 0x0a,
    REL_WHEEL_HI_RES = 0x0b,
    REL_HWHEEL_HI_RES = 0x0c,
);

impl RelativeAxisType {
    pub(crate) const COUNT: usize = REL_CNT;
}

/// A type of absolute axis measurement, typically used for touch events and joysticks.
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct AbsoluteAxisType(pub u16);

evdev_enum!(
    AbsoluteAxisType,
    Array,
    ABS_X = 0x00,
    ABS_Y = 0x01,
    ABS_Z = 0x02,
    ABS_RX = 0x03,
    ABS_RY = 0x04,
    ABS_RZ = 0x05,
    ABS_THROTTLE = 0x06,
    ABS_RUDDER = 0x07,
    ABS_WHEEL = 0x08,
    ABS_GAS = 0x09,
    ABS_BRAKE = 0x0a,
    ABS_HAT0X = 0x10,
    ABS_HAT0Y = 0x11,
    ABS_HAT1X = 0x12,
    ABS_HAT1Y = 0x13,
    ABS_HAT2X = 0x14,
    ABS_HAT2Y = 0x15,
    ABS_HAT3X = 0x16,
    ABS_HAT3Y = 0x17,
    ABS_PRESSURE = 0x18,
    ABS_DISTANCE = 0x19,
    ABS_TILT_X = 0x1a,
    ABS_TILT_Y = 0x1b,
    ABS_TOOL_WIDTH = 0x1c,
    ABS_VOLUME = 0x20,
    ABS_MISC = 0x28,
    /// "MT slot being modified"
    ABS_MT_SLOT = 0x2f,
    /// "Major axis of touching ellipse"
    ABS_MT_TOUCH_MAJOR = 0x30,
    /// "Minor axis (omit if circular)"
    ABS_MT_TOUCH_MINOR = 0x31,
    /// "Major axis of approaching ellipse"
    ABS_MT_WIDTH_MAJOR = 0x32,
    /// "Minor axis (omit if circular)"
    ABS_MT_WIDTH_MINOR = 0x33,
    /// "Ellipse orientation"
    ABS_MT_ORIENTATION = 0x34,
    /// "Center X touch position"
    ABS_MT_POSITION_X = 0x35,
    /// "Center Y touch position"
    ABS_MT_POSITION_Y = 0x36,
    /// "Type of touching device"
    ABS_MT_TOOL_TYPE = 0x37,
    /// "Group a set of packets as a blob"
    ABS_MT_BLOB_ID = 0x38,
    /// "Unique ID of the initiated contact"
    ABS_MT_TRACKING_ID = 0x39,
    /// "Pressure on contact area"
    ABS_MT_PRESSURE = 0x3a,
    /// "Contact over distance"
    ABS_MT_DISTANCE = 0x3b,
    /// "Center X tool position"
    ABS_MT_TOOL_X = 0x3c,
    /// "Center Y tool position"
    ABS_MT_TOOL_Y = 0x3d,
);

impl AbsoluteAxisType {
    pub(crate) const COUNT: usize = ABS_CNT;
}

/// An event type corresponding to a physical or virtual switch.
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct SwitchType(pub u16);

evdev_enum!(
    SwitchType,
    Array,
    /// "set = lid shut"
    SW_LID = 0x00,
    /// "set = tablet mode"
    SW_TABLET_MODE = 0x01,
    /// "set = inserted"
    SW_HEADPHONE_INSERT = 0x02,
    /// "rfkill master switch, type 'any'"
    SW_RFKILL_ALL = 0x03,
    /// "set = inserted"
    SW_MICROPHONE_INSERT = 0x04,
    /// "set = plugged into doc"
    SW_DOCK = 0x05,
    /// "set = inserted"
    SW_LINEOUT_INSERT = 0x06,
    /// "set = mechanical switch set"
    SW_JACK_PHYSICAL_INSERT = 0x07,
    /// "set  = inserted"
    SW_VIDEOOUT_INSERT = 0x08,
    /// "set = lens covered"
    SW_CAMERA_LENS_COVER = 0x09,
    /// "set = keypad slide out"
    SW_KEYPAD_SLIDE = 0x0a,
    /// "set = front proximity sensor active"
    SW_FRONT_PROXIMITY = 0x0b,
    /// "set = rotate locked/disabled"
    SW_ROTATE_LOCK = 0x0c,
    /// "set = inserted"
    SW_LINEIN_INSERT = 0x0d,
    /// "set = device disabled"
    SW_MUTE_DEVICE = 0x0e,
    /// "set = pen inserted"
    SW_PEN_INSERTED = 0x0f,
    /// "set = cover closed"
    SW_MACHINE_COVER = 0x10,
);

impl SwitchType {
    pub(crate) const COUNT: usize = SW_CNT;
}

/// LEDs specified by USB HID.
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct LedType(pub u16);

evdev_enum!(
    LedType,
    Array,
    LED_NUML = 0x00,
    LED_CAPSL = 0x01,
    LED_SCROLLL = 0x02,
    LED_COMPOSE = 0x03,
    LED_KANA = 0x04,
    /// "Stand-by"
    LED_SLEEP = 0x05,
    LED_SUSPEND = 0x06,
    LED_MUTE = 0x07,
    /// "Generic indicator"
    LED_MISC = 0x08,
    /// "Message waiting"
    LED_MAIL = 0x09,
    /// "External power connected"
    LED_CHARGING = 0x0a,
);

impl LedType {
    pub(crate) const COUNT: usize = LED_CNT;
}

/// Various miscellaneous event types.
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct MiscType(pub u16);

evdev_enum!(
    MiscType,
    Array,
    /// Serial number, only exported for tablets ("Transducer Serial Number")
    MSC_SERIAL = 0x00,
    /// Only used by the PowerMate driver, right now.
    MSC_PULSELED = 0x01,
    /// Completely unused.
    MSC_GESTURE = 0x02,
    /// "Raw" event, rarely used.
    MSC_RAW = 0x03,
    /// Key scancode
    MSC_SCAN = 0x04,
    /// Completely unused.
    MSC_TIMESTAMP = 0x05,
);

impl MiscType {
    pub(crate) const COUNT: usize = MSC_CNT;
}

/// Force feedback effect types
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct FFEffectType(pub u16);

evdev_enum!(
    FFEffectType,
    Array,
    /// Rumble effects.
    FF_RUMBLE = 0x50,
    /// Can render periodic effects with any of the waveforms.
    FF_PERIODIC = 0x51,
    /// Can render constant force effects.
    FF_CONSTANT = 0x52,
    /// Can simulate the presence of a spring.
    FF_SPRING = 0x53,
    /// Can simulate friction.
    FF_FRICTION = 0x54,
    /// Can simulate damper effects.
    FF_DAMPER = 0x55,
    /// Can simulate inertia.
    FF_INERTIA = 0x56,
    /// Can render ramp effects.
    FF_RAMP = 0x57,
    /// Square waveform.
    FF_SQUARE = 0x58,
    /// Triangle waveform.
    FF_TRIANGLE = 0x59,
    /// Sine waveform.
    FF_SINE = 0x5a,
    /// Sawtooth up waveform.
    FF_SAW_UP = 0x5b,
    /// Sawtooth down waveform.
    FF_SAW_DOWN = 0x5c,
    /// Custom waveform.
    FF_CUSTOM = 0x5d,
    /// The gain is adjustable.
    FF_GAIN = 0x60,
    /// The autocenter is adjustable.
    FF_AUTOCENTER = 0x61,
);

impl FFEffectType {
    pub(crate) const COUNT: usize = FF_CNT;
}

/// Force feedback effect status
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct FFStatus(pub u16);

evdev_enum!(
    FFStatus,
    Array,
    /// The force feedback event is currently stopped.
    FF_STATUS_STOPPED = 0x00,
    /// The force feedback event is currently playing.
    FF_STATUS_PLAYING = 0x01,
);

impl FFStatus {
    pub(crate) const COUNT: usize = 2;
}

// #[derive(Copy, Clone, PartialEq, Eq)]
// pub struct RepeatType(pub u16);

// evdev_enum!(RepeatType, REP_DELAY = 0x00, REP_PERIOD = 0x01,);

// impl RepeatType {
//     pub(crate) const COUNT: usize = libc::REP_CNT;
// }

/// A type associated with simple sounds, such as beeps or tones.
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct SoundType(pub u16);

evdev_enum!(
    SoundType,
    Array,
    SND_CLICK = 0x00,
    SND_BELL = 0x01,
    SND_TONE = 0x02,
);

impl SoundType {
    pub(crate) const COUNT: usize = SND_CNT;
}

/// A uinput event published by the kernel into the events stream for uinput devices.
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct UInputEventType(pub u16);

evdev_enum!(
    UInputEventType,
    /// The virtual uinput device is uploading a force feedback effect.
    UI_FF_UPLOAD = 1,
    /// The virtual uinput device is erasing a force feedback event.
    UI_FF_ERASE = 2,
);
