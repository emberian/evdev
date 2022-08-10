use crate::compat::{
    ff_condition_effect, ff_constant_effect, ff_periodic_effect, ff_ramp_effect, ff_replay,
    ff_rumble_effect, ff_trigger, input_absinfo, input_id, input_keymap_entry, uinput_abs_setup,
    uinput_setup,
};
use nix::{
    convert_ioctl_res, ioctl_none, ioctl_read, ioctl_read_buf, ioctl_readwrite, ioctl_write_buf,
    ioctl_write_int, ioctl_write_ptr, request_code_read,
};

#[repr(C)]
#[derive(Clone, Copy)]
pub union ff_effect_union {
    pub constant: ff_constant_effect,
    pub ramp: ff_ramp_effect,
    pub periodic: ff_periodic_effect,
    pub condition: [ff_condition_effect; 2],
    pub rumble: ff_rumble_effect,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct ff_effect {
    pub type_: u16,
    pub id: i16,
    pub direction: u16,
    pub trigger: ff_trigger,
    pub replay: ff_replay,
    pub u: ff_effect_union,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct uinput_ff_upload {
    pub request_id: u32,
    pub retval: i32,
    pub effect: ff_effect,
    pub old: ff_effect,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct uinput_ff_erase {
    pub request_id: u32,
    pub retval: i32,
    pub effect_id: u32,
}

ioctl_read!(eviocgeffects, b'E', 0x84, ::libc::c_int);
ioctl_read!(eviocgid, b'E', 0x02, /*struct*/ input_id);
ioctl_read!(eviocgkeycode, b'E', 0x04, [::libc::c_uint; 2]);
ioctl_read!(eviocgrep, b'E', 0x03, [::libc::c_uint; 2]);
ioctl_read!(eviocgversion, b'E', 0x01, ::libc::c_int);
ioctl_write_int!(eviocrmff, b'E', 0x81);

ioctl_read!(eviocgkeycode_v2, b'E', 0x04, input_keymap_entry);
ioctl_write_ptr!(eviocskeycode, b'E', 0x04, [::libc::c_uint; 2]);
ioctl_write_ptr!(eviocskeycode_v2, b'E', 0x04, input_keymap_entry);
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

const UINPUT_IOCTL_BASE: u8 = b'U';
ioctl_write_ptr!(ui_dev_setup, UINPUT_IOCTL_BASE, 3, uinput_setup);
ioctl_write_ptr!(ui_abs_setup, UINPUT_IOCTL_BASE, 4, uinput_abs_setup);
ioctl_none!(ui_dev_create, UINPUT_IOCTL_BASE, 1);

ioctl_write_int!(ui_set_evbit, UINPUT_IOCTL_BASE, 100);
ioctl_write_int!(ui_set_keybit, UINPUT_IOCTL_BASE, 101);
ioctl_write_int!(ui_set_relbit, UINPUT_IOCTL_BASE, 102);
ioctl_write_int!(ui_set_absbit, UINPUT_IOCTL_BASE, 103);
ioctl_write_int!(ui_set_mscbit, UINPUT_IOCTL_BASE, 104);
ioctl_write_int!(ui_set_ledbit, UINPUT_IOCTL_BASE, 105);
ioctl_write_int!(ui_set_sndbit, UINPUT_IOCTL_BASE, 106);
ioctl_write_int!(ui_set_ffbit, UINPUT_IOCTL_BASE, 107);
ioctl_write_buf!(ui_set_phys, UINPUT_IOCTL_BASE, 108, u8);
ioctl_write_int!(ui_set_swbit, UINPUT_IOCTL_BASE, 109);
ioctl_write_int!(ui_set_propbit, UINPUT_IOCTL_BASE, 110);

pub unsafe fn ui_get_sysname(fd: ::libc::c_int, bytes: &mut [u8]) -> ::nix::Result<::libc::c_int> {
    convert_ioctl_res!(::nix::libc::ioctl(
        fd,
        request_code_read!(UINPUT_IOCTL_BASE, 300, bytes.len()),
        bytes.as_mut_ptr(),
    ))
}

ioctl_readwrite!(ui_begin_ff_upload, UINPUT_IOCTL_BASE, 200, uinput_ff_upload);
ioctl_write_ptr!(ui_end_ff_upload, UINPUT_IOCTL_BASE, 201, uinput_ff_upload);
ioctl_readwrite!(ui_begin_ff_erase, UINPUT_IOCTL_BASE, 202, uinput_ff_erase);
ioctl_write_ptr!(ui_end_ff_erase, UINPUT_IOCTL_BASE, 203, uinput_ff_erase);

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
) -> ::nix::Result<::libc::c_int> {
    assert!(abs <= 0x3f);
    convert_ioctl_res!(::nix::libc::ioctl(
        fd,
        request_code_read!(b'E', 0x40 + abs, ::std::mem::size_of::<input_absinfo>()),
        buf as *mut input_absinfo
    ))
}
