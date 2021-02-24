use libc::c_int;
pub use libc::{
    ff_condition_effect, ff_constant_effect, ff_effect, ff_envelope, ff_periodic_effect,
    ff_ramp_effect, ff_replay, ff_rumble_effect, ff_trigger, input_absinfo, input_event, input_id,
    input_keymap_entry,
};
use nix::{
    convert_ioctl_res, ioctl_read, ioctl_read_buf, ioctl_write_int, ioctl_write_ptr,
    request_code_read,
};

pub(crate) const fn input_absinfo_default() -> input_absinfo {
    input_absinfo {
        value: 0,
        minimum: 0,
        maximum: 0,
        fuzz: 0,
        flat: 0,
        resolution: 0,
    }
}

ioctl_read!(eviocgeffects, b'E', 0x84, ::libc::c_int);
ioctl_read!(eviocgid, b'E', 0x02, /*struct*/ input_id);
ioctl_read!(eviocgkeycode, b'E', 0x04, [::libc::c_uint; 2]);
ioctl_read!(eviocgrep, b'E', 0x03, [::libc::c_uint; 2]);
ioctl_read!(eviocgversion, b'E', 0x01, ::libc::c_int);
ioctl_write_int!(eviocrmff, b'E', 0x81);
// ioctl!(read eviocgkeycode_v2 with b'E', 0x04; /*struct*/ input_keymap_entry);
// TODO #define EVIOCSFF _IOC ( _IOC_WRITE , 'E' , 0x80 , sizeof ( struct ff_effect ) )
ioctl_write_ptr!(eviocskeycode, b'E', 0x04, [::libc::c_uint; 2]);
// ioctl!(write_int eviocskeycode_v2 with b'E', 0x04; /*struct*/ input_keymap_entry);
ioctl_write_ptr!(eviocsrep, b'E', 0x03, [::libc::c_uint; 2]);

ioctl_read_buf!(eviocgname, b'E', 0x06, u8);
ioctl_read_buf!(eviocgphys, b'E', 0x07, u8);
ioctl_read_buf!(eviocguniq, b'E', 0x08, u8);
ioctl_read_buf!(eviocgprop, b'E', 0x09, u8);
ioctl_read_buf!(eviocgmtslots, b'E', 0x0a, u8);
ioctl_read_buf!(eviocgkey, b'E', 0x18, u8);
ioctl_read_buf!(eviocgled, b'E', 0x19, u8);
ioctl_read_buf!(eviocgsnd, b'E', 0x1a, u8);
ioctl_read_buf!(eviocgsw, b'E', 0x1b, u8);

ioctl_write_ptr!(eviocsff, b'E', 0x80, ff_effect);
ioctl_write_int!(eviocgrab, b'E', 0x90);
ioctl_write_int!(eviocrevoke, b'E', 0x91);
ioctl_write_int!(eviocsclockid, b'E', 0xa0);

macro_rules! eviocgbit_ioctl {
    ($mac:ident!($name:ident, $ev:ident, $ty:ty)) => {
        eviocgbit_ioctl!($mac!($name, $crate::EventType::$ev.0, $ty));
    };
    ($mac:ident!($name:ident, $ev:expr, $ty:ty)) => {
        $mac!($name, b'E', 0x20 + $ev, $ty);
    };
}

eviocgbit_ioctl!(ioctl_read_buf!(eviocgbit_type, 0, u8));
eviocgbit_ioctl!(ioctl_read_buf!(eviocgbit_key, KEY, u8));
eviocgbit_ioctl!(ioctl_read_buf!(eviocgbit_relative, RELATIVE, u8));
eviocgbit_ioctl!(ioctl_read_buf!(eviocgbit_absolute, ABSOLUTE, u8));
eviocgbit_ioctl!(ioctl_read_buf!(eviocgbit_misc, MISC, u8));
eviocgbit_ioctl!(ioctl_read_buf!(eviocgbit_switch, SWITCH, u8));
eviocgbit_ioctl!(ioctl_read_buf!(eviocgbit_led, LED, u8));
eviocgbit_ioctl!(ioctl_read_buf!(eviocgbit_sound, SOUND, u8));
eviocgbit_ioctl!(ioctl_read_buf!(eviocgbit_repeat, REPEAT, u8));
eviocgbit_ioctl!(ioctl_read_buf!(eviocgbit_ff, FORCEFEEDBACK, u8));
eviocgbit_ioctl!(ioctl_read_buf!(eviocgbit_power, POWER, u8));
eviocgbit_ioctl!(ioctl_read_buf!(eviocgbit_ffstatus, FORCEFEEDBACKSTATUS, u8));

/// ioctl: "get abs value/limits"
///
/// `abs` should be one of the "Absolute axes" values defined in the Linux kernel headers.
/// In modern (5.11) kernels these are in `include/uapi/linux/input-event-codes.h`, and in older
/// kernels these defines can be found in `include/uapi/linux/input.h`
///
/// # Panics
///
/// Calling this with a value greater than the kernel-defined `ABS_MAX` (typically 0x3f) will panic.
///
/// # Safety
///
/// 'abs' must be a valid axis number and supported by the device, otherwise the behavior is
/// undefined.
pub unsafe fn eviocgabs(
    fd: ::libc::c_int,
    abs: u32,
    buf: &mut input_absinfo,
) -> ::nix::Result<c_int> {
    assert!(abs <= 0x3f);
    convert_ioctl_res!(::nix::libc::ioctl(
        fd,
        request_code_read!(b'E', 0x40 + abs, ::std::mem::size_of::<input_absinfo>()),
        buf as *mut input_absinfo
    ))
}
