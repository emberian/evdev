//! Linux event device handling.
//!
//! The Linux kernel's "evdev" subsystem exposes input devices to userspace in a generic,
//! consistent way. I'll try to explain the device model as completely as possible. The upstream
//! kernel documentation is split across two files:
//!
//! - https://www.kernel.org/doc/Documentation/input/event-codes.txt
//! - https://www.kernel.org/doc/Documentation/input/multi-touch-protocol.txt
//!
//! Devices can expose a few different kinds of events, specified by the `Types` bitflag. Each
//! event type (except for RELATIVE and SYNCHRONIZATION) also has some associated state. See the documentation for
//! `Types` on what each type corresponds to.
//!
//! This state can be queried. For example, the `DeviceState::led_vals` field will tell you which
//! LEDs are currently lit on the device. This state is not automatically synchronized with the
//! kernel. However, as the application reads events, this state will be updated if the event is
//! newer than the state timestamp (maintained internally).  Additionally, you can call
//! `Device::sync_state` to explicitly synchronize with the kernel state.
//!
//! As the state changes, the kernel will write events into a ring buffer. The application can read
//! from this ring buffer, thus retreiving events. However, if the ring buffer becomes full, the
//! kernel will *drop* every event in the ring buffer and leave an event telling userspace that it
//! did so. At this point, if the application were using the events it received to update its
//! internal idea of what state the hardware device is in, it will be wrong: it is missing some
//! events. This library tries to ease that pain, but it is best-effort. Events can never be
//! recovered once lost. For example, if a switch is toggled twice, there will be two switch events
//! in the buffer. However if the kernel needs to drop events, when the device goes to synchronize
//! state with the kernel, only one (or zero, if the switch is in the same state as it was before
//! the sync) switch events will be emulated.
//!
//! It is recommended that you dedicate a thread to processing input events, or use epoll with the
//! fd returned by `Device::fd` to process events when they are ready.

#![cfg(any(target_os = "linux", target_os = "android"))]

#[macro_use]
extern crate bitflags;
extern crate ioctl;
extern crate libc;
extern crate errno;
extern crate fixedbitset;
extern crate num;

use std::os::unix::io::*;
use std::os::unix::ffi::*;
use std::path::Path;
use std::ffi::CString;
use std::mem::size_of;
use fixedbitset::FixedBitSet;

pub use Key::*;
pub use FFEffect::*;
pub use Synchronization::*;

#[link(name = "rt")]
extern {
    fn clock_gettime(clkid: libc::c_int, res: *mut libc::timespec);
}

#[derive(Debug)]
pub enum Error {
    NulError(std::ffi::NulError),
    LibcError(errno::Errno),
    IoctlError(&'static str, errno::Errno),
}

impl From<std::ffi::NulError> for Error {
    fn from(e: std::ffi::NulError) -> Error {
        Error::NulError(e)
    }
}

impl From<errno::Errno> for Error {
    fn from(e: errno::Errno) -> Error {
        Error::LibcError(e)
    }
}

macro_rules! do_ioctl {
    ($name:ident($($arg:expr),+)) => {{
        let rc = unsafe { ::ioctl::$name($($arg,)+) };
        if rc < 0 {
            return Err(Error::IoctlError(stringify!($name), errno::errno()))
        }
        rc
    }}
}

struct Fd(libc::c_int);
impl Drop for Fd {
    fn drop(&mut self) {
        unsafe { libc::close(self.0); }
    }
}
impl std::ops::Deref for Fd {
    type Target = libc::c_int;
    fn deref(&self) -> &libc::c_int {
        &self.0
    }
}

bitflags! {
    /// Event types supported by the device.
    pub flags Types: u32 {
        /// A bookkeeping event. Usually not important to applications.
        const SYNCHRONIZATION = 1 << 0x00,
        /// A key changed state. A key, or button, is usually a momentary switch (in the circuit sense). It has two
        /// states: down, or up. There are events for when keys are pressed (become down) and
        /// released (become up). There are also "key repeats", where multiple events are sent
        /// while a key is down.
        const KEY = 1 << 0x01,
        /// Movement on a relative axis. There is no absolute coordinate frame, just the fact that
        /// there was a change of a certain amount of units. Used for things like mouse movement or
        /// scroll wheels.
        const RELATIVE = 1 << 0x02,
        /// Movement on an absolute axis. Used for things such as touch events and joysticks.
        const ABSOLUTE = 1 << 0x03,
        /// Miscellaneous events that don't fall into other categories. I'm not quite sure when
        /// these happen or what they correspond to.
        const MISC = 1 << 0x04,
        /// Change in a switch value. Switches are boolean conditions and usually correspond to a
        /// toggle switch of some kind in hardware.
        const SWITCH = 1 << 0x05,
        /// An LED was toggled.
        const LED = 1 << 0x11,
        /// A sound was made.
        const SOUND = 1 << 0x12,
        /// There are no events of this type, to my knowledge, but represents metadata about key
        /// repeat configuration.
        const REPEAT = 1 << 0x14,
        /// I believe there are no events of this type, but rather this is used to represent that
        /// the device can create haptic effects.
        const FORCEFEEDBACK = 1 << 0x15,
        /// I think this is unused?
        const POWER = 1 << 0x16,
        /// A force feedback effect's state changed.
        const FORCEFEEDBACKSTATUS = 1 << 0x17,
    }
}

bitflags! {
    /// Device properties.
    pub flags Props: u32 {
        /// This input device needs a pointer ("cursor") for the user to know its state.
        const POINTER = 1 << 0x00,
        /// "direct input devices", according to the header.
        const DIRECT = 1 << 0x01,
        /// "has button(s) under pad", according to the header.
        const BUTTONPAD = 1 << 0x02,
        /// Touch rectangle only (I think this means that if there are multiple touches, then the
        /// bounding rectangle of all the touches is returned, not each touch).
        const SEMI_MT = 1 << 0x03,
        /// "softbuttons at top of pad", according to the header.
        const TOPBUTTONPAD = 1 << 0x04,
        /// Is a pointing stick ("clit mouse" etc, https://xkcd.com/243/)
        const POINTING_STICK = 1 << 0x05,
        /// Has an accelerometer. Probably reports relative events in that case?
        const ACCELEROMETER = 1 << 0x06
    }
}

include!("scancodes.rs"); // it's a huge glob of text that I'm tired of skipping over.

bitflags! {
    pub flags RelativeAxis: u32 {
        const REL_X = 1 << 0x00,
        const REL_Y = 1 << 0x01,
        const REL_Z = 1 << 0x02,
        const REL_RX = 1 << 0x03,
        const REL_RY = 1 << 0x04,
        const REL_RZ = 1 << 0x05,
        const REL_HWHEEL = 1 << 0x06,
        const REL_DIAL = 1 << 0x07,
        const REL_WHEEL = 1 << 0x08,
        const REL_MISC = 1 << 0x09,
    }
}

bitflags! {
    pub flags AbsoluteAxis: u64 {
        const ABS_X = 1 << 0x00,
        const ABS_Y = 1 << 0x01,
        const ABS_Z = 1 << 0x02,
        const ABS_RX = 1 << 0x03,
        const ABS_RY = 1 << 0x04,
        const ABS_RZ = 1 << 0x05,
        const ABS_THROTTLE = 1 << 0x06,
        const ABS_RUDDER = 1 << 0x07,
        const ABS_WHEEL = 1 << 0x08,
        const ABS_GAS = 1 << 0x09,
        const ABS_BRAKE = 1 << 0x0a,
        const ABS_HAT0X = 1 << 0x10,
        const ABS_HAT0Y = 1 << 0x11,
        const ABS_HAT1X = 1 << 0x12,
        const ABS_HAT1Y = 1 << 0x13,
        const ABS_HAT2X = 1 << 0x14,
        const ABS_HAT2Y = 1 << 0x15,
        const ABS_HAT3X = 1 << 0x16,
        const ABS_HAT3Y = 1 << 0x17,
        const ABS_PRESSURE = 1 << 0x18,
        const ABS_DISTANCE = 1 << 0x19,
        const ABS_TILT_X = 1 << 0x1a,
        const ABS_TILT_Y = 1 << 0x1b,
        const ABS_TOOL_WIDTH = 1 << 0x1c,
        const ABS_VOLUME = 1 << 0x20,
        const ABS_MISC = 1 << 0x28,
        /// "MT slot being modified"
        const ABS_MT_SLOT = 1 << 0x2f,
        /// "Major axis of touching ellipse"
        const ABS_MT_TOUCH_MAJOR = 1 << 0x30,
        /// "Minor axis (omit if circular)"
        const ABS_MT_TOUCH_MINOR = 1 << 0x31,
        /// "Major axis of approaching ellipse"
        const ABS_MT_WIDTH_MAJOR = 1 << 0x32,
        /// "Minor axis (omit if circular)"
        const ABS_MT_WIDTH_MINOR = 1 << 0x33,
        /// "Ellipse orientation"
        const ABS_MT_ORIENTATION = 1 << 0x34,
        /// "Center X touch position"
        const ABS_MT_POSITION_X = 1 << 0x35,
        /// "Center Y touch position"
        const ABS_MT_POSITION_Y = 1 << 0x36,
        /// "Type of touching device"
        const ABS_MT_TOOL_TYPE = 1 << 0x37,
        /// "Group a set of packets as a blob"
        const ABS_MT_BLOB_ID = 1 << 0x38,
        /// "Unique ID of the initiated contact"
        const ABS_MT_TRACKING_ID = 1 << 0x39,
        /// "Pressure on contact area"
        const ABS_MT_PRESSURE = 1 << 0x3a,
        /// "Contact over distance"
        const ABS_MT_DISTANCE = 1 << 0x3b,
        /// "Center X tool position"
        const ABS_MT_TOOL_X = 1 << 0x3c,
        /// "Center Y tool position"
        const ABS_MT_TOOL_Y = 1 << 0x3d,
        const ABS_MAX = 1 << 0x3f,
    }
}

bitflags! {
    pub flags Switch: u32 {
        /// "set = lid shut"
        const SW_LID = 1 << 0x00,
        /// "set = tablet mode"
        const SW_TABLET_MODE = 1 << 0x01,
        /// "set = inserted"
        const SW_HEADPHONE_INSERT = 1 << 0x02,
        /// "rfkill master switch, type 'any'"
        const SW_RFKILL_ALL = 1 << 0x03,
        /// "set = inserted"
        const SW_MICROPHONE_INSERT = 1 << 0x04,
        /// "set = plugged into doc"
        const SW_DOCK = 1 << 0x05,
        /// "set = inserted"
        const SW_LINEOUT_INSERT = 1 << 0x06,
        /// "set = mechanical switch set"
        const SW_JACK_PHYSICAL_INSERT = 1 << 0x07,
        /// "set  = inserted"
        const SW_VIDEOOUT_INSERT = 1 << 0x08,
        /// "set = lens covered"
        const SW_CAMERA_LENS_COVER = 1 << 0x09,
        /// "set = keypad slide out"
        const SW_KEYPAD_SLIDE = 1 << 0x0a,
        /// "set = front proximity sensor active"
        const SW_FRONT_PROXIMITY = 1 << 0x0b,
        /// "set = rotate locked/disabled"
        const SW_ROTATE_LOCK = 1 << 0x0c,
        /// "set = inserted"
        const SW_LINEIN_INSERT = 1 << 0x0d,
        /// "set = device disabled"
        const SW_MUTE_DEVICE = 1 << 0x0e,
        const SW_MAX = 1 << 0x0f,
    }
}

bitflags! {
    /// LEDs specified by USB HID.
    pub flags Led: u32 {
        const LED_NUML = 1 << 0x00,
        const LED_CAPSL = 1 << 0x01,
        const LED_SCROLLL = 1 << 0x02,
        const LED_KANA = 1 << 0x04,
        /// "Stand-by"
        const LED_SLEEP = 1 << 0x05,
        const LED_SUSPEND = 1 << 0x06,
        const LED_MUTE = 1 << 0x07,
        /// "Generic indicator"
        const LED_MISC = 1 << 0x08,
        /// "Message waiting"
        const LED_MAIL = 1 << 0x09,
        /// "External power connected"
        const LED_CHARGING = 1 << 0x0a,
        const LED_MAX = 1 << 0x0f,
    }
}

bitflags! {
    /// Various miscellaneous event types. Current as of kernel 4.1.
    pub flags Misc: u32 {
        /// Serial number, only exported for tablets ("Transducer Serial Number")
        const MSC_SERIAL = 1 << 0x00,
        /// Only used by the PowerMate driver, right now.
        const MSC_PULSELED = 1 << 0x01,
        /// Completely unused.
        const MSC_GESTURE = 1 << 0x02,
        /// "Raw" event, rarely used.
        const MSC_RAW = 1 << 0x03,
        /// Key scancode
        const MSC_SCAN = 1 << 0x04,
        /// Completely unused.
        const MSC_TIMESTAMP = 1 << 0x05,
        const MSC_MAX = 1 << 0x07,
    }
}

bitflags! {
    flags FFStatus: u32 {
        const FF_STATUS_STOPPED	= 1 << 0x00,
        const FF_STATUS_PLAYING	= 1 << 0x01,
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub enum FFEffect {
    FF_RUMBLE = 0x50,
    FF_PERIODIC = 0x51,
    FF_CONSTANT = 0x52,
    FF_SPRING = 0x53,
    FF_FRICTION = 0x54,
    FF_DAMPER = 0x55,
    FF_INERTIA = 0x56,
    FF_RAMP = 0x57,
    FF_SQUARE = 0x58,
    FF_TRIANGLE = 0x59,
    FF_SINE = 0x5a,
    FF_SAW_UP = 0x5b,
    FF_SAW_DOWN = 0x5c,
    FF_CUSTOM = 0x5d,
    FF_GAIN = 0x60,
    FF_AUTOCENTER = 0x61,
    FF_MAX = 0x7f,
}

bitflags! {
    pub flags Repeat: u32 {
        const REP_DELAY = 1 << 0x00,
        const REP_PERIOD = 1 << 0x01,
    }
}

bitflags! {
    pub flags Sound: u32 {
        const SND_CLICK = 1 << 0x00,
        const SND_BELL = 1 << 0x01,
        const SND_TONE = 1 << 0x02,
    }
}

macro_rules! impl_number {
    ($($t:ident),*) => {
        $(impl $t {
            /// Given a bitflag with only a single flag set, returns the event code corresponding to that
            /// event. If multiple flags are set, the one with the most significant bit wins. In debug
            /// mode,
            #[inline(always)]
            pub fn number<T: num::FromPrimitive>(&self) -> T {
                if cfg!(debug_assertions) {
                    let val = ffs(self.bits());
                    self.bits() == val; // hack to induce the constraint typeof(self.bits()) = typeof(val)
                    if self.bits() != 1 << val {
                        panic!("{:?} ought to have only one flag set to be used with .number()", self);
                    }
                }
                T::from_u32(ffs(self.bits())).unwrap()
            }
        })*
    }
}

impl_number!(Types, Props, RelativeAxis, AbsoluteAxis, Switch, Led, Misc, FFStatus, Repeat, Sound);

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub enum Synchronization {
    /// Terminates a packet of events from the device.
    SYN_REPORT = 0,
    /// Appears to be unused.
    SYN_CONFIG = 1,
    /// "Used to synchronize and separate touch events"
    SYN_MT_REPORT = 2,
    /// Ring buffer filled, events were dropped.
    SYN_DROPPED = 3,
    SYN_MAX = 0xf,
}

#[derive(Clone)]
pub struct DeviceState {
    /// The state corresponds to kernel state at this timestamp.
    pub timestamp: libc::timeval,
    /// Set = key pressed
    pub key_vals: FixedBitSet,
    pub abs_vals: Vec<ioctl::input_absinfo>,
    /// Set = switch enabled (closed)
    pub switch_vals: FixedBitSet,
    /// Set = LED lit
    pub led_vals: FixedBitSet,
}

pub struct Device {
    fd: RawFd,
    ty: Types,
    name: CString,
    phys: Option<CString>,
    uniq: Option<CString>,
    id: ioctl::input_id,
    props: Props,
    driver_version: (u8, u8, u8),
    key_bits: FixedBitSet,
    rel: RelativeAxis,
    abs: AbsoluteAxis,
    switch: Switch,
    led: Led,
    misc: Misc,
    ff: FixedBitSet,
    ff_stat: FFStatus,
    rep: Repeat,
    snd: Sound,
    pending_events: Vec<ioctl::input_event>,
    clock: libc::c_int,
    // pending_events[last_seen..] is the events that have occurred since the last sync.
    last_seen: usize,
    state: DeviceState,
}

impl std::fmt::Debug for Device {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let mut ds = f.debug_struct("Device");
        ds.field("name", &self.name).field("fd", &self.fd).field("ty", &self.ty);
        if let Some(ref phys) = self.phys {
            ds.field("phys", phys);
        }
        if let Some(ref uniq) = self.uniq {
            ds.field("uniq", uniq);
        }
        ds.field("id", &self.id)
          .field("id", &self.id)
          .field("props", &self.props)
          .field("driver_version", &self.driver_version);
        if self.ty.contains(SYNCHRONIZATION) {

        }
        if self.ty.contains(KEY) {
            ds.field("key_bits", &self.key_bits)
              .field("key_vals", &self.state.key_vals);
        }
        if self.ty.contains(RELATIVE) {
            ds.field("rel", &self.rel);
        }
        if self.ty.contains(ABSOLUTE) {
            ds.field("abs", &self.abs);
            for idx in (0..0x3f) {
                let abs = 1 << idx;
                // ignore multitouch, we'll handle that later.
                if (self.abs.bits() & abs) == 1 {
                    // eugh.
                    ds.field(&format!("abs_{:x}", idx), &self.state.abs_vals[idx as usize]);
                }
            }
        }
        if self.ty.contains(MISC) {

        }
        if self.ty.contains(SWITCH) {
            ds.field("switch", &self.switch)
              .field("switch_vals", &self.state.switch_vals);
        }
        if self.ty.contains(LED) {
            ds.field("led", &self.led)
              .field("led_vals", &self.state.led_vals);
        }
        if self.ty.contains(SOUND) {
            ds.field("snd", &self.snd);
        }
        if self.ty.contains(REPEAT) {
            ds.field("rep", &self.rep);
        }
        if self.ty.contains(FORCEFEEDBACK) {
            ds.field("ff", &self.ff);
        }
        if self.ty.contains(POWER) {
        }
        if self.ty.contains(FORCEFEEDBACKSTATUS) {
            ds.field("ff_stat", &self.ff_stat);
        }
        ds.finish()
    }
}

fn bus_name(x: u16) -> &'static str {
    match x {
        0x1 => "PCI",
        0x2 => "ISA Plug 'n Play",
        0x3 => "USB",
        0x4 => "HIL",
        0x5 => "Bluetooth",
        0x6 => "Virtual",
        0x10 => "ISA",
        0x11 => "i8042",
        0x12 => "XTKBD",
        0x13 => "RS232",
        0x14 => "Gameport",
        0x15 => "Parallel Port",
        0x16 => "Amiga",
        0x17 => "ADB",
        0x18 => "I2C",
        0x19 => "Host",
        0x1A => "GSC",
        0x1B => "Atari",
        0x1C => "SPI",
        _ => "Unknown",
    }
}

impl std::fmt::Display for Device {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        try!(writeln!(f, "{:?}", self.name));
        try!(writeln!(f, "  Driver version: {}.{}.{}", self.driver_version.0, self.driver_version.1, self.driver_version.2));
        if let Some(ref phys) = self.phys {
            try!(writeln!(f, "  Physical address: {:?}", phys));
        }
        if let Some(ref uniq) = self.uniq {
            try!(writeln!(f, "  Unique name: {:?}", uniq));
        }

        try!(writeln!(f, "  Bus: {}", bus_name(self.id.bustype)));
        try!(writeln!(f, "  Vendor: 0x{:x}", self.id.vendor));
        try!(writeln!(f, "  Product: 0x{:x}", self.id.product));
        try!(writeln!(f, "  Version: 0x{:x}", self.id.version));
        try!(writeln!(f, "  Properties: {:?}", self.props));

        if self.ty.contains(SYNCHRONIZATION) {

        }

        if self.ty.contains(KEY) {
            try!(writeln!(f, "  Keys supported:"));
            for key_idx in (0..self.key_bits.len()) {
                if self.key_bits.contains(key_idx) {
                    // Cross our fingers... (what did this mean?)
                    try!(writeln!(f, "    {:?} ({}index {})",
                                 unsafe { std::mem::transmute::<_, Key>(key_idx as libc::c_int) },
                                 if self.state.key_vals.contains(key_idx) { "pressed, " } else { "" },
                                 key_idx));
                }
            }
        }
        if self.ty.contains(RELATIVE) {
            try!(writeln!(f, "  Relative Axes: {:?}", self.rel));
        }
        if self.ty.contains(ABSOLUTE) {
            try!(writeln!(f, "  Absolute Axes:"));
            for idx in (0..0x3f) {
                let abs = 1<< idx;
                if self.abs.bits() & abs != 0 {
                    // FIXME: abs val Debug is gross
                    try!(writeln!(f, "    {:?} ({:?}, index {})",
                         AbsoluteAxis::from_bits(abs).unwrap(),
                         self.state.abs_vals[idx as usize],
                         idx));
                }
            }
        }
        if self.ty.contains(MISC) {
            try!(writeln!(f, "  Miscellaneous capabilities: {:?}", self.misc));
        }
        if self.ty.contains(SWITCH) {
            try!(writeln!(f, "  Switches:"));
            for idx in (0..0xf) {
                let sw = 1 << idx;
                if sw < SW_MAX.bits() && self.switch.bits() & sw == 1 {
                    try!(writeln!(f, "    {:?} ({:?}, index {})",
                         Switch::from_bits(sw).unwrap(),
                         self.state.switch_vals[idx as usize],
                         idx));
                }
            }
        }
        if self.ty.contains(LED) {
            try!(writeln!(f, "  LEDs:"));
            for idx in (0..0xf) {
                let led = 1 << idx;
                if led < LED_MAX.bits() && self.led.bits() & led == 1 {
                    try!(writeln!(f, "    {:?} ({:?}, index {})",
                         Led::from_bits(led).unwrap(),
                         self.state.led_vals[idx as usize],
                         idx));
                }
            }
        }
        if self.ty.contains(SOUND) {
            try!(writeln!(f, "  Sound: {:?}", self.snd));
        }
        if self.ty.contains(REPEAT) {
            try!(writeln!(f, "  Repeats: {:?}", self.rep));
        }
        if self.ty.contains(FORCEFEEDBACK) {
            try!(writeln!(f, "  Force Feedback supported"));
        }
        if self.ty.contains(POWER) {
            try!(writeln!(f, "  Power supported"));
        }
        if self.ty.contains(FORCEFEEDBACKSTATUS) {
            try!(writeln!(f, "  Force Feedback status supported"));
        }
        Ok(())
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        // Linux close(2) can fail, but there is nothing to do if it does.
        unsafe { libc::close(self.fd); }
    }
}

fn ffs<T: num::FromPrimitive, U: num::ToPrimitive>(x: U) -> T {
    T::from_u32(31 - U::to_u64(&x).unwrap().leading_zeros()).unwrap()
}

impl Device {
    pub fn fd(&self) -> RawFd {
        self.fd
    }

    pub fn events_supported(&self) -> Types {
        self.ty
    }

    pub fn name(&self) -> &CString {
        &self.name
    }

    pub fn physical_path(&self) -> &Option<CString> {
        &self.phys
    }

    pub fn unique_name(&self) -> &Option<CString> {
        &self.uniq
    }

    pub fn input_id(&self) -> ioctl::input_id {
        self.id
    }

    pub fn properties(&self) -> Props {
        self.props
    }

    pub fn driver_version(&self) -> (u8, u8, u8) {
        self.driver_version
    }

    pub fn keys_supported(&self) -> &FixedBitSet {
        &self.key_bits
    }

    pub fn relative_axes_supported(&self) -> RelativeAxis {
        self.rel
    }

    pub fn absolute_axes_supported(&self) -> AbsoluteAxis {
        self.abs
    }

    pub fn switches_supported(&self) -> Switch {
        self.switch
    }

    pub fn leds_supported(&self) -> Led {
        self.led
    }

    pub fn misc_properties(&self) -> Misc {
        self.misc
    }

    pub fn repeats_supported(&self) -> Repeat {
        self.rep
    }

    pub fn sounds_supported(&self) -> Sound {
        self.snd
    }

    pub fn state(&self) -> &DeviceState {
        &self.state
    }

    pub fn open(path: &AsRef<Path>) -> Result<Device, Error> {
        let cstr = match CString::new(path.as_ref().as_os_str().as_bytes()) {
            Ok(s) => s,
            Err(e) => return Err(Error::NulError(e))
        };
        // FIXME: only need for writing is for setting LED values. re-evaluate always using RDWR
        // later.
        let fd = Fd(unsafe { libc::open(cstr.as_ptr(), libc::O_NONBLOCK | libc::O_RDWR, 0) });
        if *fd == -1 {
            std::mem::forget(fd);
            return Err(Error::LibcError(errno::errno()))
        }
        do_ioctl!(fioclex(*fd)); // non-atomic :( but no O_CLOEXEC yet.

        let mut dev = Device {
            fd: *fd,
            ty: Types::empty(),
            name: unsafe { CString::from_vec_unchecked(Vec::new()) },
            phys: None,
            uniq: None,
            id: unsafe { std::mem::zeroed() },
            props: Props::empty(),
            driver_version: (0, 0, 0),
            key_bits: FixedBitSet::with_capacity(KEY_MAX as usize),
            rel: RelativeAxis::empty(),
            abs: AbsoluteAxis::empty(),
            switch: Switch::empty(),
            led: Led::empty(),
            misc: Misc::empty(),
            ff: FixedBitSet::with_capacity(FF_MAX as usize + 1),
            ff_stat: FFStatus::empty(),
            rep: Repeat::empty(),
            snd: Sound::empty(),
            pending_events: Vec::with_capacity(64),
            last_seen: 0,
            state: DeviceState {
                timestamp: libc::timeval { tv_sec: 0, tv_usec: 0 },
                key_vals: FixedBitSet::with_capacity(KEY_MAX as usize),
                abs_vals: vec![],
                switch_vals: FixedBitSet::with_capacity(0x10),
                led_vals: FixedBitSet::with_capacity(0x10),
            },
            clock: libc::CLOCK_REALTIME
        };

        let mut bits: u32 = 0;
        let mut bits64: u64 = 0;
        let mut vec = Vec::with_capacity(256);

        do_ioctl!(eviocgbit(*fd, 0, 4, &mut bits as *mut u32 as *mut u8));
        dev.ty = Types::from_bits(bits).expect("evdev: unexpected type bits! report a bug");

        let dev_len = do_ioctl!(eviocgname(*fd, vec.as_mut_ptr(), 255));
        unsafe { vec.set_len(dev_len as usize - 1) };
        dev.name = CString::new(vec.clone()).unwrap();

        let phys_len = unsafe { ioctl::eviocgphys(*fd, vec.as_mut_ptr(), 255) };
        if phys_len > 0 {
            unsafe { vec.set_len(phys_len as usize - 1) };
            dev.phys = Some(CString::new(vec.clone()).unwrap());
        }

        let uniq_len = unsafe { ioctl::eviocguniq(*fd, vec.as_mut_ptr(), 255) };
        if uniq_len > 0 {
            unsafe { vec.set_len(uniq_len as usize - 1) };
            dev.uniq = Some(CString::new(vec.clone()).unwrap());
        }

        do_ioctl!(eviocgid(*fd, &mut dev.id));
        let mut driver_version: i32 = 0;
        do_ioctl!(eviocgversion(*fd, &mut driver_version));
        dev.driver_version =
            (((driver_version >> 16) & 0xff) as u8,
             ((driver_version >> 8) & 0xff) as u8,
              (driver_version & 0xff) as u8);

        do_ioctl!(eviocgprop(*fd, &mut bits as *mut u32 as *mut u8, 0x1f)); // FIXME: handle old kernel
        dev.props = Props::from_bits(bits).expect("evdev: unexpected prop bits! report a bug");

        if dev.ty.contains(KEY) {
            do_ioctl!(eviocgbit(*fd, KEY.number(), dev.key_bits.len() as libc::c_int, dev.key_bits.as_mut_slice().as_mut_ptr() as *mut u8));
        }

        if dev.ty.contains(RELATIVE) {
            do_ioctl!(eviocgbit(*fd, RELATIVE.number(), 0xf, &mut bits as *mut u32 as *mut u8));
            dev.rel = RelativeAxis::from_bits(bits).expect("evdev: unexpected rel bits! report a bug");
        }

        if dev.ty.contains(ABSOLUTE) {
            do_ioctl!(eviocgbit(*fd, ABSOLUTE.number(), 0x3f, &mut bits64 as *mut u64 as *mut u8));
            println!("abs bits: {:b}", bits64);
            dev.abs = AbsoluteAxis::from_bits(bits64).expect("evdev: unexpected abs bits! report a bug");
            dev.state.abs_vals = vec![ioctl::input_absinfo::default(); 0x3f];
        }

        if dev.ty.contains(SWITCH) {
            do_ioctl!(eviocgbit(*fd, SWITCH.number(), 0xf, &mut bits as *mut u32 as *mut u8));
            dev.switch = Switch::from_bits(bits).expect("evdev: unexpected switch bits! report a bug");
        }

        if dev.ty.contains(LED) {
            do_ioctl!(eviocgbit(*fd, LED.number(), 0xf, &mut bits as *mut u32 as *mut u8));
            dev.led = Led::from_bits(bits).expect("evdev: unexpected led bits! report a bug");
        }

        if dev.ty.contains(MISC) {
            do_ioctl!(eviocgbit(*fd, MISC.number(), 0x7, &mut bits as *mut u32 as *mut u8));
            dev.misc = Misc::from_bits(bits).expect("evdev: unexpected misc bits! report a bug");
        }

        //do_ioctl!(eviocgbit(*fd, ffs(FORCEFEEDBACK.bits()), 0x7f, &mut bits as *mut u32 as *mut u8));

        if dev.ty.contains(SOUND) {
            do_ioctl!(eviocgbit(*fd, SOUND.number(), 0x7, &mut bits as *mut u32 as *mut u8));
            dev.snd = Sound::from_bits(bits).expect("evdev: unexpected sound bits! report a bug");
        }

        try!(dev.sync_state());

        std::mem::forget(fd);
        Ok(dev)
    }

    /// Synchronize the `Device` state with the kernel device state.
    ///
    /// If there is an error at any point, the state will not be synchronized completely.
    pub fn sync_state(&mut self) -> Result<(), Error> {
        if self.ty.contains(KEY) {
            do_ioctl!(eviocgkey(self.fd, self.state.key_vals.as_mut_slice().as_mut_ptr() as *mut u32 as *mut u8, self.state.key_vals.len()));
        }
        if self.ty.contains(ABSOLUTE) {
            for idx in (0..0x28) {
                let abs = 1 << idx;
                // ignore multitouch, we'll handle that later.
                if abs < ABS_MT_SLOT.bits() && self.abs.bits() & abs != 1 {
                    do_ioctl!(eviocgabs(self.fd, idx as u32, &mut self.state.abs_vals[idx as usize]));
                }
            }
        }
        if self.ty.contains(SWITCH) {
            do_ioctl!(eviocgsw(self.fd, self.state.switch_vals.as_mut_slice().as_mut_ptr() as *mut u32 as *mut u8, self.state.switch_vals.len()));
        }
        if self.ty.contains(LED) {
            do_ioctl!(eviocgled(self.fd, self.state.led_vals.as_mut_slice().as_mut_ptr() as *mut u32 as *mut u8, self.state.led_vals.len()));
        }

        Ok(())
    }

    /// Do SYN_DROPPED synchronization, and compensate for missing events by inserting events into
    /// the stream which, when applied to any state being kept outside of this `Device`, will
    /// synchronize it with the kernel state.
    fn compensate_dropped(&mut self) -> Result<(), Error> {
        let mut drop_from = None;
        for (idx, event) in self.pending_events[self.last_seen..].iter().enumerate() {
            if event._type == SYN_DROPPED as u16 {
                drop_from = Some(idx);
                break
            }
        }
        // FIXME: see if we can *not* drop EV_REL events. EV_REL doesn't have any state, so
        // dropping its events isn't really helping much.
        if let Some(idx) = drop_from {
            // look for the nearest SYN_REPORT before the SYN_DROPPED, remove everything after it.
            let mut prev_report = 0; // (if there's no previous SYN_REPORT, then the entire vector is bogus)
            for (idx, event) in self.pending_events[..idx].iter().enumerate().rev() {
                if event._type == SYN_REPORT as u16 {
                    prev_report = idx;
                    break;
                }
            }
            self.pending_events.truncate(prev_report);
        }

        // Alright, pending_events is in a sane state. Now, let's sync the local state. We will
        // create a phony packet that contains deltas from the previous device state to the current
        // device state.
        let old_state = self.state.clone();
        try!(self.sync_state());
        let mut time = unsafe { std::mem::zeroed() };
        unsafe { clock_gettime(self.clock, &mut time); }
        let time = libc::timeval {
            tv_sec: time.tv_sec,
            tv_usec: time.tv_nsec * 1000,
        };

        if self.ty.contains(KEY) {
            for key_idx in (0..self.key_bits.len()) {
                if self.key_bits.contains(key_idx) {
                    if old_state.key_vals[key_idx] != self.state.key_vals[key_idx] {
                        self.pending_events.push(ioctl::input_event {
                            time: time,
                            _type: KEY.number(),
                            code: key_idx as u16,
                            value: if self.state.key_vals[key_idx] { 1 } else { 0 },
                        });
                    }
                }
            }
        }
        if self.ty.contains(ABSOLUTE) {
            for idx in (0..0x3f) {
                let abs = 1 << idx;
                if self.abs.bits() & abs != 0 {
                    if old_state.abs_vals[idx as usize] != self.state.abs_vals[idx as usize] {
                        self.pending_events.push(ioctl::input_event {
                            time: time,
                            _type: ABSOLUTE.number(),
                            code: idx as u16,
                            value: self.state.abs_vals[idx as usize].value,
                        });
                    }
                }
            }
        }
        if self.ty.contains(SWITCH) {
            for idx in (0..0xf) {
                let sw = 1 << idx;
                if sw < SW_MAX.bits() && self.switch.bits() & sw == 1 {
                    if old_state.switch_vals[idx as usize] != self.state.switch_vals[idx as usize] {
                        self.pending_events.push(ioctl::input_event {
                            time: time,
                            _type: SWITCH.number(),
                            code: idx as u16,
                            value: if self.state.switch_vals[idx as usize] { 1 } else { 0 },
                        });
                    }
                }
            }
        }
        if self.ty.contains(LED) {
            for idx in (0..0xf) {
                let led = 1 << idx;
                if led < LED_MAX.bits() && self.led.bits() & led == 1 {
                    if old_state.led_vals[idx as usize] != self.state.led_vals[idx as usize] {
                        self.pending_events.push(ioctl::input_event {
                            time: time,
                            _type: LED.number(),
                            code: idx as u16,
                            value: if self.state.led_vals[idx as usize] { 1 } else { 0 },
                        });
                    }
                }
            }
        }

        self.pending_events.push(ioctl::input_event {
            time: time,
            _type: SYNCHRONIZATION.number(),
            code: SYN_REPORT as u16,
            value: 0,
        });
        Ok(())
    }

    fn fill_events(&mut self) -> Result<(), Error> {
        let mut buf = &mut self.pending_events;
        loop {
            buf.reserve(20);
            let pre_len = buf.len();
            let sz = unsafe {
                libc::read(self.fd,
                           buf.as_mut_ptr()
                              .offset(pre_len as isize) as *mut libc::c_void,
                           (size_of::<ioctl::input_event>() * (buf.capacity() - pre_len)) as libc::size_t)
            };
            if sz == -1 {
                let errno = errno::errno();
                if errno != errno::Errno(libc::EAGAIN) {
                    return Err(Error::LibcError(errno));
                } else {
                    break;
                }
            } else {
                unsafe {
                    buf.set_len(pre_len + (sz as usize / size_of::<ioctl::input_event>()));
                }
            }
        }
        Ok(())
    }

    /// Exposes the raw evdev events without doing synchronization on SYN_DROPPED.
    pub fn events_no_sync(&mut self) -> Result<RawEvents, Error> {
        try!(self.fill_events());
        Ok(RawEvents::new(self))
    }

    /// Exposes the raw evdev events, doing synchronization on SYN_DROPPED.
    ///
    /// Will insert "fake" events
    pub fn events(&mut self) -> Result<RawEvents, Error> {
        try!(self.fill_events());
        try!(self.compensate_dropped());

        Ok(RawEvents(self))
    }
}

pub struct Events<'a>(&'a mut Device);

pub struct RawEvents<'a>(&'a mut Device);

impl<'a> RawEvents<'a> {
    fn new(dev: &'a mut Device) -> RawEvents<'a> {
        dev.pending_events.reverse();
        RawEvents(dev)
    }
}

impl<'a> Drop for RawEvents<'a> {
    fn drop(&mut self) {
        self.0.pending_events.reverse();
        self.0.last_seen = self.0.pending_events.len();
    }
}

impl<'a> Iterator for RawEvents<'a> {
    type Item = ioctl::input_event;

    #[inline(always)]
    fn next(&mut self) -> Option<ioctl::input_event> {
        self.0.pending_events.pop()
    }
}

/// Crawls `/dev/input` for evdev devices.
///
/// Will not bubble up any errors in opening devices or traversing the directory. Instead returns
/// an empty vector or omits the devices that could not be opened.
pub fn enumerate() -> Vec<Device> {
    let mut res = Vec::new();
    if let Ok(dir) = std::fs::read_dir("/dev/input") {
        for entry in dir {
            if let Ok(entry) = entry {
                if let Ok(dev) = Device::open(&entry.path()) {
                    res.push(dev)
                }
            }
        }
    }
    res
}

#[cfg(test)] mod test { include!("tests.rs"); }
