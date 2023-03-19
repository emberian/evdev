//! FreeBSD and other non-Linux targets don't have these available in libc, because they're in
//! the "linux-like" impl directory. They are copied here for convenience and compatibility.
//!
//! BSD-likes are only minimally supported by evdev. Use at your own risk.

#![allow(non_camel_case_types)]

pub const FF_MAX: u16 = 0x7f;
pub const FF_CNT: usize = FF_MAX as usize + 1;
pub const INPUT_PROP_MAX: u16 = 0x1f;
pub const INPUT_PROP_CNT: usize = INPUT_PROP_MAX as usize + 1;
pub const EV_MAX: u16 = 0x1f;
pub const EV_CNT: usize = EV_MAX as usize + 1;
pub const KEY_MAX: u16 = 0x2ff;
pub const KEY_CNT: usize = KEY_MAX as usize + 1;
pub const REL_MAX: u16 = 0x0f;
pub const REL_CNT: usize = REL_MAX as usize + 1;
pub const ABS_MAX: u16 = 0x3f;
pub const ABS_CNT: usize = ABS_MAX as usize + 1;
pub const SW_MAX: u16 = 0x10;
pub const SW_CNT: usize = SW_MAX as usize + 1;
pub const MSC_MAX: u16 = 0x07;
pub const MSC_CNT: usize = MSC_MAX as usize + 1;
pub const LED_MAX: u16 = 0x0f;
pub const LED_CNT: usize = LED_MAX as usize + 1;
pub const SND_MAX: u16 = 0x07;
pub const SND_CNT: usize = SND_MAX as usize + 1;
pub const UINPUT_MAX_NAME_SIZE: usize = 80;

#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct input_event {
    pub time: libc::timeval,
    pub type_: u16,
    pub code: u16,
    pub value: i32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct input_id {
    pub bustype: u16,
    pub vendor: u16,
    pub product: u16,
    pub version: u16,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct input_absinfo {
    pub value: i32,
    pub minimum: i32,
    pub maximum: i32,
    pub fuzz: i32,
    pub flat: i32,
    pub resolution: i32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct input_keymap_entry {
    pub flags: u8,
    pub len: u8,
    pub index: u16,
    pub keycode: u32,
    pub scancode: [u8; 32],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct ff_replay {
    pub length: u16,
    pub delay: u16,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct ff_trigger {
    pub button: u16,
    pub interval: u16,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct ff_envelope {
    pub attack_length: u16,
    pub attack_level: u16,
    pub fade_length: u16,
    pub fade_level: u16,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct ff_constant_effect {
    pub level: i16,
    pub envelope: ff_envelope,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct ff_ramp_effect {
    pub start_level: i16,
    pub end_level: i16,
    pub envelope: ff_envelope,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct ff_condition_effect {
    pub right_saturation: u16,
    pub left_saturation: u16,

    pub right_coeff: i16,
    pub left_coeff: i16,

    pub deadband: u16,
    pub center: i16,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct ff_periodic_effect {
    pub waveform: u16,
    pub period: u16,
    pub magnitude: i16,
    pub offset: i16,
    pub phase: u16,

    pub envelope: ff_envelope,

    pub custom_len: u32,
    pub custom_data: *mut i16,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct ff_rumble_effect {
    pub strong_magnitude: u16,
    pub weak_magnitude: u16,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct ff_effect {
    pub type_: u16,
    pub id: i16,
    pub direction: u16,
    pub trigger: ff_trigger,
    pub replay: ff_replay,
    // FIXME this is actually a union
    #[cfg(target_pointer_width = "64")]
    pub u: [u64; 4],
    #[cfg(target_pointer_width = "32")]
    pub u: [u32; 7],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct uinput_abs_setup {
    pub code: u16,
    pub absinfo: input_absinfo,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub struct uinput_setup {
    pub id: input_id,
    pub name: [libc::c_char; UINPUT_MAX_NAME_SIZE],
    pub ff_effects_max: u32,
}
