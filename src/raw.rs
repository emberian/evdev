ioctl!(read eviocgeffects with b'E', 0x84; ::libc::c_int);
ioctl!(read eviocgid with b'E', 0x02; /*struct*/ input_id);
ioctl!(read eviocgkeycode with b'E', 0x04; [::libc::c_uint; 2]);
ioctl!(read eviocgrep with b'E', 0x03; [::libc::c_uint; 2]);
ioctl!(read eviocgversion with b'E', 0x01; ::libc::c_int);
ioctl!(write_int eviocrmff with b'E', 0x81);
// ioctl!(read eviocgkeycode_v2 with b'E', 0x04; /*struct*/ input_keymap_entry);
// TODO #define EVIOCSFF _IOC ( _IOC_WRITE , 'E' , 0x80 , sizeof ( struct ff_effect ) )
ioctl!(write_ptr eviocskeycode with b'E', 0x04; [::libc::c_uint; 2]);
// ioctl!(write_int eviocskeycode_v2 with b'E', 0x04; /*struct*/ input_keymap_entry);
ioctl!(write_ptr eviocsrep with b'E', 0x03; [::libc::c_uint; 2]);

#[repr(C)]
#[derive(Copy, Clone)]
pub struct input_event {
    pub time: ::libc::timeval,
    pub _type: u16,
    pub code: u16,
    pub value: i32,
}
impl ::std::default::Default for input_event {
    fn default() -> Self {
        unsafe { ::std::mem::zeroed() }
    }
}
impl ::std::fmt::Debug for input_event {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        write!(
            f,
            "input_event {{ time: {{ tv_sec: {}, tv_usec: {} }}, _type: {}, code: {}, value: {}",
            self.time.tv_sec, self.time.tv_usec, self._type, self.code, self.value
        )
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct input_id {
    pub bustype: u16,
    pub vendor: u16,
    pub product: u16,
    pub version: u16,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ff_effect {
    pub _type: u16,
    pub id: i16,
    pub direction: u16,
    pub trigger: ff_trigger,
    pub replay: ff_replay,
    pub u: Union_Unnamed16,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct Union_Unnamed16 {
    pub _bindgen_data_: [u64; 4usize],
}
impl Union_Unnamed16 {
    pub unsafe fn constant(&mut self) -> *mut ff_constant_effect {
        let raw: *mut u8 = ::std::mem::transmute(&self._bindgen_data_);
        ::std::mem::transmute(raw.offset(0))
    }
    pub unsafe fn ramp(&mut self) -> *mut ff_ramp_effect {
        let raw: *mut u8 = ::std::mem::transmute(&self._bindgen_data_);
        ::std::mem::transmute(raw.offset(0))
    }
    pub unsafe fn periodic(&mut self) -> *mut ff_periodic_effect {
        let raw: *mut u8 = ::std::mem::transmute(&self._bindgen_data_);
        ::std::mem::transmute(raw.offset(0))
    }
    pub unsafe fn condition(&mut self) -> *mut [ff_condition_effect; 2usize] {
        let raw: *mut u8 = ::std::mem::transmute(&self._bindgen_data_);
        ::std::mem::transmute(raw.offset(0))
    }
    pub unsafe fn rumble(&mut self) -> *mut ff_rumble_effect {
        let raw: *mut u8 = ::std::mem::transmute(&self._bindgen_data_);
        ::std::mem::transmute(raw.offset(0))
    }
}
impl ::std::default::Default for Union_Unnamed16 {
    fn default() -> Self {
        unsafe { ::std::mem::zeroed() }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub struct input_absinfo {
    pub value: i32,
    pub minimum: i32,
    pub maximum: i32,
    pub fuzz: i32,
    pub flat: i32,
    pub resolution: i32,
}
impl ::std::default::Default for input_absinfo {
    fn default() -> Self {
        unsafe { ::std::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct input_keymap_entry {
    pub flags: u8,
    pub len: u8,
    pub index: u16,
    pub keycode: u32,
    pub scancode: [u8; 32usize],
}
impl ::std::default::Default for input_keymap_entry {
    fn default() -> Self {
        unsafe { ::std::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct ff_replay {
    pub length: u16,
    pub delay: u16,
}
impl ::std::default::Default for ff_replay {
    fn default() -> Self {
        unsafe { ::std::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct ff_trigger {
    pub button: u16,
    pub interval: u16,
}
impl ::std::default::Default for ff_trigger {
    fn default() -> Self {
        unsafe { ::std::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct ff_envelope {
    pub attack_length: u16,
    pub attack_level: u16,
    pub fade_length: u16,
    pub fade_level: u16,
}
impl ::std::default::Default for ff_envelope {
    fn default() -> Self {
        unsafe { ::std::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct ff_constant_effect {
    pub level: i16,
    pub envelope: ff_envelope,
}
impl ::std::default::Default for ff_constant_effect {
    fn default() -> Self {
        unsafe { ::std::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct ff_ramp_effect {
    pub start_level: i16,
    pub end_level: i16,
    pub envelope: ff_envelope,
}
impl ::std::default::Default for ff_ramp_effect {
    fn default() -> Self {
        unsafe { ::std::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct ff_condition_effect {
    pub right_saturation: u16,
    pub left_saturation: u16,
    pub right_coeff: i16,
    pub left_coeff: i16,
    pub deadband: u16,
    pub center: i16,
}
impl ::std::default::Default for ff_condition_effect {
    fn default() -> Self {
        unsafe { ::std::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Copy, Clone, Debug)]
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
impl ::std::default::Default for ff_periodic_effect {
    fn default() -> Self {
        unsafe { ::std::mem::zeroed() }
    }
}
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct ff_rumble_effect {
    pub strong_magnitude: u16,
    pub weak_magnitude: u16,
}
impl ::std::default::Default for ff_rumble_effect {
    fn default() -> Self {
        unsafe { ::std::mem::zeroed() }
    }
}

ioctl!(read_buf eviocgname with b'E', 0x06; u8);
ioctl!(read_buf eviocgphys with b'E', 0x07; u8);
ioctl!(read_buf eviocguniq with b'E', 0x08; u8);
ioctl!(read_buf eviocgprop with b'E', 0x09; u8);
ioctl!(read_buf eviocgmtslots with b'E', 0x0a; u8);
ioctl!(read_buf eviocgkey with b'E', 0x18; u8);
ioctl!(read_buf eviocgled with b'E', 0x19; u8);
ioctl!(read_buf eviocgsnd with b'E', 0x1a; u8);
ioctl!(read_buf eviocgsw with b'E', 0x1b; u8);

ioctl!(write_ptr eviocsff with b'E', 0x80; ff_effect);
ioctl!(write_int eviocgrab with b'E', 0x90);
ioctl!(write_int eviocrevoke with b'E', 0x91);
ioctl!(write_int eviocsclockid with b'E', 0xa0);

pub unsafe fn eviocgbit(
    fd: ::libc::c_int,
    ev: u32,
    len: ::libc::c_int,
    buf: *mut u8,
) -> ::nix::Result<i32> {
    convert_ioctl_res!(::nix::libc::ioctl(
        fd,
        ior!(b'E', 0x20 + ev, len) as ::libc::c_ulong,
        buf
    ))
}

pub unsafe fn eviocgabs(
    fd: ::libc::c_int,
    abs: u32,
    buf: *mut input_absinfo,
) -> ::nix::Result<i32> {
    convert_ioctl_res!(::nix::libc::ioctl(
        fd,
        ior!(b'E', 0x40 + abs, ::std::mem::size_of::<input_absinfo>()) as ::libc::c_ulong,
        buf
    ))
}
