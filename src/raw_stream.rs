use std::fs::{File, OpenOptions};
use std::io::Write;
use std::mem::MaybeUninit;
use std::os::unix::io::{AsRawFd, RawFd};
use std::path::Path;
use std::{io, mem};

use crate::compat::{input_absinfo, input_event, input_id, input_keymap_entry};
use crate::constants::*;
use crate::ff::*;
use crate::{sys, AttributeSet, AttributeSetRef, FFEffectType, InputEvent, InputId, Key};

fn ioctl_get_cstring(
    f: unsafe fn(RawFd, &mut [u8]) -> nix::Result<libc::c_int>,
    fd: RawFd,
) -> Option<Vec<u8>> {
    let mut buf = vec![0; 256];
    match unsafe { f(fd, buf.as_mut_slice()) } {
        Ok(len) if len as usize > buf.capacity() => {
            panic!("ioctl_get_cstring call overran the provided buffer!");
        }
        Ok(len) if len > 1 => {
            // Our ioctl string functions apparently return the number of bytes written, including
            // trailing \0.
            buf.truncate(len as usize);
            assert_eq!(buf.pop().unwrap(), 0);
            Some(buf)
        }
        _ => None,
    }
}

fn bytes_into_string_lossy(v: Vec<u8>) -> String {
    String::from_utf8(v).unwrap_or_else(|v| String::from_utf8_lossy(v.as_bytes()).into_owned())
}

const ABSINFO_ZERO: input_absinfo = input_absinfo {
    value: 0,
    minimum: 0,
    maximum: 0,
    fuzz: 0,
    flat: 0,
    resolution: 0,
};

pub(crate) const ABS_VALS_INIT: [input_absinfo; AbsoluteAxisType::COUNT] =
    [ABSINFO_ZERO; AbsoluteAxisType::COUNT];

const INPUT_KEYMAP_BY_INDEX: u8 = 1;

/// Represents a force feedback effect that has been successfully uploaded to the device for
/// playback.
#[derive(Debug)]
pub struct FFEffect {
    file: File,
    id: u16,
}

impl FFEffect {
    /// Returns the effect ID.
    pub fn id(&self) -> u16 {
        self.id
    }

    /// Plays the force feedback effect with the `count` argument specifying how often the effect
    /// should be played.
    pub fn play(&mut self, count: i32) -> io::Result<()> {
        let events = [InputEvent::new(EventType::FORCEFEEDBACK, self.id, count)];
        let bytes = unsafe { crate::cast_to_bytes(&events) };
        self.file.write_all(bytes)?;

        Ok(())
    }

    /// Stops playback of the force feedback effect.
    pub fn stop(&mut self) -> io::Result<()> {
        let events = [InputEvent::new(EventType::FORCEFEEDBACK, self.id, 0)];
        let bytes = unsafe { crate::cast_to_bytes(&events) };
        self.file.write_all(bytes)?;

        Ok(())
    }

    /// Updates the force feedback effect.
    pub fn update(&mut self, data: FFEffectData) -> io::Result<()> {
        let mut effect: sys::ff_effect = data.into();
        effect.id = self.id as i16;

        unsafe { sys::eviocsff(self.file.as_raw_fd(), &effect)? };

        Ok(())
    }
}

impl Drop for FFEffect {
    fn drop(&mut self) {
        let _ = unsafe { sys::eviocrmff(self.file.as_raw_fd(), self.id as _) };
    }
}

/// A physical or virtual device supported by evdev.
///
/// Each device corresponds to a path typically found in `/dev/input`, and supports access via
/// one or more "types". For example, an optical mouse has buttons that are represented by "keys",
/// and reflects changes in its position via "relative axis" reports.
#[derive(Debug)]
pub struct RawDevice {
    file: File,
    ty: AttributeSet<EventType>,
    name: Option<String>,
    phys: Option<String>,
    uniq: Option<String>,
    id: input_id,
    props: AttributeSet<PropType>,
    driver_version: (u8, u8, u8),
    supported_keys: Option<AttributeSet<Key>>,
    supported_relative: Option<AttributeSet<RelativeAxisType>>,
    supported_absolute: Option<AttributeSet<AbsoluteAxisType>>,
    supported_switch: Option<AttributeSet<SwitchType>>,
    supported_led: Option<AttributeSet<LedType>>,
    supported_misc: Option<AttributeSet<MiscType>>,
    supported_ff: Option<AttributeSet<FFEffectType>>,
    auto_repeat: Option<AutoRepeat>,
    // ff: Option<AttributeSet<_>>,
    // ff_stat: Option<FFStatus>,
    supported_snd: Option<AttributeSet<SoundType>>,
    pub(crate) event_buf: Vec<input_event>,
    grabbed: bool,
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct AutoRepeat {
    pub delay: u32,
    pub period: u32,
}

impl RawDevice {
    /// Opens a device, given its system path.
    ///
    /// Paths are typically something like `/dev/input/event0`.
    #[inline(always)]
    pub fn open(path: impl AsRef<Path>) -> io::Result<RawDevice> {
        Self::_open(path.as_ref())
    }

    fn _open(path: &Path) -> io::Result<RawDevice> {
        let mut options = OpenOptions::new();

        // Try to load read/write, then fall back to read-only.
        let file = options
            .read(true)
            .write(true)
            .open(path)
            .or_else(|_| options.write(false).open(path))?;

        let ty = {
            let mut ty = AttributeSet::<EventType>::new();
            unsafe { sys::eviocgbit_type(file.as_raw_fd(), ty.as_mut_raw_slice())? };
            ty
        };

        let name =
            ioctl_get_cstring(sys::eviocgname, file.as_raw_fd()).map(bytes_into_string_lossy);
        let phys =
            ioctl_get_cstring(sys::eviocgphys, file.as_raw_fd()).map(bytes_into_string_lossy);
        let uniq =
            ioctl_get_cstring(sys::eviocguniq, file.as_raw_fd()).map(bytes_into_string_lossy);

        let id = unsafe {
            let mut id = MaybeUninit::uninit();
            sys::eviocgid(file.as_raw_fd(), id.as_mut_ptr())?;
            id.assume_init()
        };
        let mut driver_version: i32 = 0;
        unsafe {
            sys::eviocgversion(file.as_raw_fd(), &mut driver_version)?;
        }
        let driver_version = (
            ((driver_version >> 16) & 0xff) as u8,
            ((driver_version >> 8) & 0xff) as u8,
            (driver_version & 0xff) as u8,
        );

        let props = {
            let mut props = AttributeSet::<PropType>::new();
            unsafe { sys::eviocgprop(file.as_raw_fd(), props.as_mut_raw_slice())? };
            props
        }; // FIXME: handle old kernel

        let supported_keys = if ty.contains(EventType::KEY) {
            let mut keys = AttributeSet::<Key>::new();
            unsafe { sys::eviocgbit_key(file.as_raw_fd(), keys.as_mut_raw_slice())? };
            Some(keys)
        } else {
            None
        };

        let supported_relative = if ty.contains(EventType::RELATIVE) {
            let mut rel = AttributeSet::<RelativeAxisType>::new();
            unsafe { sys::eviocgbit_relative(file.as_raw_fd(), rel.as_mut_raw_slice())? };
            Some(rel)
        } else {
            None
        };

        let supported_absolute = if ty.contains(EventType::ABSOLUTE) {
            let mut abs = AttributeSet::<AbsoluteAxisType>::new();
            unsafe { sys::eviocgbit_absolute(file.as_raw_fd(), abs.as_mut_raw_slice())? };
            Some(abs)
        } else {
            None
        };

        let supported_switch = if ty.contains(EventType::SWITCH) {
            let mut switch = AttributeSet::<SwitchType>::new();
            unsafe { sys::eviocgbit_switch(file.as_raw_fd(), switch.as_mut_raw_slice())? };
            Some(switch)
        } else {
            None
        };

        let supported_led = if ty.contains(EventType::LED) {
            let mut led = AttributeSet::<LedType>::new();
            unsafe { sys::eviocgbit_led(file.as_raw_fd(), led.as_mut_raw_slice())? };
            Some(led)
        } else {
            None
        };

        let supported_misc = if ty.contains(EventType::MISC) {
            let mut misc = AttributeSet::<MiscType>::new();
            unsafe { sys::eviocgbit_misc(file.as_raw_fd(), misc.as_mut_raw_slice())? };
            Some(misc)
        } else {
            None
        };

        let supported_ff = if ty.contains(EventType::FORCEFEEDBACK) {
            let mut ff = AttributeSet::<FFEffectType>::new();
            unsafe { sys::eviocgbit_ff(file.as_raw_fd(), ff.as_mut_raw_slice())? };
            Some(ff)
        } else {
            None
        };

        let supported_snd = if ty.contains(EventType::SOUND) {
            let mut snd = AttributeSet::<SoundType>::new();
            unsafe { sys::eviocgbit_sound(file.as_raw_fd(), snd.as_mut_raw_slice())? };
            Some(snd)
        } else {
            None
        };

        let auto_repeat = if ty.contains(EventType::REPEAT) {
            let mut auto_repeat: AutoRepeat = AutoRepeat {
                delay: 0,
                period: 0,
            };

            unsafe {
                sys::eviocgrep(
                    file.as_raw_fd(),
                    &mut auto_repeat as *mut AutoRepeat as *mut [u32; 2],
                )?;
            }

            Some(auto_repeat)
        } else {
            None
        };

        Ok(RawDevice {
            file,
            ty,
            name,
            phys,
            uniq,
            id,
            props,
            driver_version,
            supported_keys,
            supported_relative,
            supported_absolute,
            supported_switch,
            supported_led,
            supported_misc,
            supported_ff,
            supported_snd,
            auto_repeat,
            event_buf: Vec::new(),
            grabbed: false,
        })
    }

    /// Returns the device's name as read from the kernel.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Returns the device's physical location, either as set by the caller or as read from the kernel.
    pub fn physical_path(&self) -> Option<&str> {
        self.phys.as_deref()
    }

    /// Returns the user-defined "unique name" of the device, if one has been set.
    pub fn unique_name(&self) -> Option<&str> {
        self.uniq.as_deref()
    }

    /// Returns a struct containing bustype, vendor, product, and version identifiers
    pub fn input_id(&self) -> InputId {
        InputId::from(self.id)
    }

    /// Returns the current auto repeat settings
    pub fn get_auto_repeat(&self) -> Option<AutoRepeat> {
        self.auto_repeat.clone()
    }

    /// Returns the set of supported "properties" for the device (see `INPUT_PROP_*` in kernel headers)
    pub fn properties(&self) -> &AttributeSetRef<PropType> {
        &self.props
    }

    /// Returns a tuple of the driver version containing major, minor, rev
    pub fn driver_version(&self) -> (u8, u8, u8) {
        self.driver_version
    }

    /// Returns a set of the event types supported by this device (Key, Switch, etc)
    ///
    /// If you're interested in the individual keys or switches supported, it's probably easier
    /// to just call the appropriate `supported_*` function instead.
    pub fn supported_events(&self) -> &AttributeSetRef<EventType> {
        &self.ty
    }

    /// Returns the set of supported keys reported by the device.
    ///
    /// For keyboards, this is the set of all possible keycodes the keyboard may emit. Controllers,
    /// mice, and other peripherals may also report buttons as keys.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use evdev::{Device, Key};
    /// let device = Device::open("/dev/input/event0")?;
    ///
    /// // Does this device have an ENTER key?
    /// let supported = device.supported_keys().map_or(false, |keys| keys.contains(Key::KEY_ENTER));
    /// # Ok(())
    /// # }
    /// ```
    pub fn supported_keys(&self) -> Option<&AttributeSetRef<Key>> {
        self.supported_keys.as_deref()
    }

    /// Returns the set of supported "relative axes" reported by the device.
    ///
    /// Standard mice will generally report `REL_X` and `REL_Y` along with wheel if supported.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use evdev::{Device, RelativeAxisType};
    /// let device = Device::open("/dev/input/event0")?;
    ///
    /// // Does the device have a scroll wheel?
    /// let supported = device
    ///     .supported_relative_axes()
    ///     .map_or(false, |axes| axes.contains(RelativeAxisType::REL_WHEEL));
    /// # Ok(())
    /// # }
    /// ```
    pub fn supported_relative_axes(&self) -> Option<&AttributeSetRef<RelativeAxisType>> {
        self.supported_relative.as_deref()
    }

    /// Returns the set of supported "absolute axes" reported by the device.
    ///
    /// These are most typically supported by joysticks and touchpads.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use evdev::{Device, AbsoluteAxisType};
    /// let device = Device::open("/dev/input/event0")?;
    ///
    /// // Does the device have an absolute X axis?
    /// let supported = device
    ///     .supported_absolute_axes()
    ///     .map_or(false, |axes| axes.contains(AbsoluteAxisType::ABS_X));
    /// # Ok(())
    /// # }
    /// ```
    pub fn supported_absolute_axes(&self) -> Option<&AttributeSetRef<AbsoluteAxisType>> {
        self.supported_absolute.as_deref()
    }

    /// Returns the set of supported switches reported by the device.
    ///
    /// These are typically used for things like software switches on laptop lids (which the
    /// system reacts to by suspending or locking), or virtual switches to indicate whether a
    /// headphone jack is plugged in (used to disable external speakers).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use evdev::{Device, SwitchType};
    /// let device = Device::open("/dev/input/event0")?;
    ///
    /// // Does the device report a laptop lid switch?
    /// let supported = device
    ///     .supported_switches()
    ///     .map_or(false, |axes| axes.contains(SwitchType::SW_LID));
    /// # Ok(())
    /// # }
    /// ```
    pub fn supported_switches(&self) -> Option<&AttributeSetRef<SwitchType>> {
        self.supported_switch.as_deref()
    }

    /// Returns a set of supported LEDs on the device.
    ///
    /// Most commonly these are state indicator lights for things like Scroll Lock, but they
    /// can also be found in cameras and other devices.
    pub fn supported_leds(&self) -> Option<&AttributeSetRef<LedType>> {
        self.supported_led.as_deref()
    }

    /// Returns a set of supported "miscellaneous" capabilities.
    ///
    /// Aside from vendor-specific key scancodes, most of these are uncommon.
    pub fn misc_properties(&self) -> Option<&AttributeSetRef<MiscType>> {
        self.supported_misc.as_deref()
    }

    /// Returns the set of supported force feedback effects supported by a device.
    pub fn supported_ff(&self) -> Option<&AttributeSetRef<FFEffectType>> {
        self.supported_ff.as_deref()
    }

    /// Returns the set of supported simple sounds supported by a device.
    ///
    /// You can use these to make really annoying beep sounds come from an internal self-test
    /// speaker, for instance.
    pub fn supported_sounds(&self) -> Option<&AttributeSetRef<SoundType>> {
        self.supported_snd.as_deref()
    }

    /// Read a maximum of `num` events into the internal buffer. If the underlying fd is not
    /// O_NONBLOCK, this will block.
    ///
    /// Returns the number of events that were read, or an error.
    pub(crate) fn fill_events(&mut self) -> io::Result<usize> {
        let fd = self.as_raw_fd();
        self.event_buf.reserve(crate::EVENT_BATCH_SIZE);

        // TODO: use Vec::spare_capacity_mut or Vec::split_at_spare_mut when they stabilize
        let spare_capacity = vec_spare_capacity_mut(&mut self.event_buf);
        let spare_capacity_size = std::mem::size_of_val(spare_capacity);

        // use libc::read instead of nix::unistd::read b/c we need to pass an uninitialized buf
        let res = unsafe { libc::read(fd, spare_capacity.as_mut_ptr() as _, spare_capacity_size) };
        let bytes_read = nix::errno::Errno::result(res)?;
        let num_read = bytes_read as usize / mem::size_of::<input_event>();
        unsafe {
            let len = self.event_buf.len();
            self.event_buf.set_len(len + num_read);
        }
        Ok(num_read)
    }

    /// Fetches and returns events from the kernel ring buffer without doing synchronization on
    /// SYN_DROPPED.
    ///
    /// By default this will block until events are available. Typically, users will want to call
    /// this in a tight loop within a thread.
    pub fn fetch_events(&mut self) -> io::Result<impl Iterator<Item = InputEvent> + '_> {
        self.fill_events()?;
        Ok(self.event_buf.drain(..).map(InputEvent))
    }

    /// Retrieve the current keypress state directly via kernel syscall.
    #[inline]
    pub fn get_key_state(&self) -> io::Result<AttributeSet<Key>> {
        let mut key_vals = AttributeSet::new();
        self.update_key_state(&mut key_vals)?;
        Ok(key_vals)
    }

    /// Retrieve the current absolute axis state directly via kernel syscall.
    #[inline]
    pub fn get_abs_state(&self) -> io::Result<[input_absinfo; AbsoluteAxisType::COUNT]> {
        let mut abs_vals: [input_absinfo; AbsoluteAxisType::COUNT] = ABS_VALS_INIT;
        self.update_abs_state(&mut abs_vals)?;
        Ok(abs_vals)
    }

    /// Retrieve the current switch state directly via kernel syscall.
    #[inline]
    pub fn get_switch_state(&self) -> io::Result<AttributeSet<SwitchType>> {
        let mut switch_vals = AttributeSet::new();
        self.update_switch_state(&mut switch_vals)?;
        Ok(switch_vals)
    }

    /// Retrieve the current LED state directly via kernel syscall.
    #[inline]
    pub fn get_led_state(&self) -> io::Result<AttributeSet<LedType>> {
        let mut led_vals = AttributeSet::new();
        self.update_led_state(&mut led_vals)?;
        Ok(led_vals)
    }

    /// Fetch the current kernel key state directly into the provided buffer.
    /// If you don't already have a buffer, you probably want
    /// [`get_key_state`](Self::get_key_state) instead.
    #[inline]
    pub fn update_key_state(&self, key_vals: &mut AttributeSet<Key>) -> io::Result<()> {
        unsafe { sys::eviocgkey(self.as_raw_fd(), key_vals.as_mut_raw_slice())? };
        Ok(())
    }

    /// Fetch the current kernel absolute axis state directly into the provided buffer.
    /// If you don't already have a buffer, you probably want
    /// [`get_abs_state`](Self::get_abs_state) instead.
    #[inline]
    pub fn update_abs_state(
        &self,
        abs_vals: &mut [input_absinfo; AbsoluteAxisType::COUNT],
    ) -> io::Result<()> {
        if let Some(supported_abs) = self.supported_absolute_axes() {
            for AbsoluteAxisType(idx) in supported_abs.iter() {
                // ignore multitouch, we'll handle that later.
                //
                // handling later removed. not sure what the intention of "handling that later" was
                // the abs data seems to be fine (tested ABS_MT_POSITION_X/Y)
                unsafe {
                    sys::eviocgabs(self.as_raw_fd(), idx as u32, &mut abs_vals[idx as usize])?
                };
            }
        }
        Ok(())
    }

    /// Fetch the current kernel switch state directly into the provided buffer.
    /// If you don't already have a buffer, you probably want
    /// [`get_switch_state`](Self::get_switch_state) instead.
    #[inline]
    pub fn update_switch_state(
        &self,
        switch_vals: &mut AttributeSet<SwitchType>,
    ) -> io::Result<()> {
        unsafe { sys::eviocgsw(self.as_raw_fd(), switch_vals.as_mut_raw_slice())? };
        Ok(())
    }

    /// Fetch the current kernel LED state directly into the provided buffer.
    /// If you don't already have a buffer, you probably want
    /// [`get_led_state`](Self::get_led_state) instead.
    #[inline]
    pub fn update_led_state(&self, led_vals: &mut AttributeSet<LedType>) -> io::Result<()> {
        unsafe { sys::eviocgled(self.as_raw_fd(), led_vals.as_mut_raw_slice())? };
        Ok(())
    }

    /// Update the auto repeat delays
    #[inline]
    pub fn update_auto_repeat(&mut self, repeat: &AutoRepeat) -> io::Result<()> {
        unsafe {
            sys::eviocsrep(
                self.as_raw_fd(),
                repeat as *const AutoRepeat as *const [u32; 2],
            )?;
        }
        self.auto_repeat = Some(repeat.clone());
        Ok(())
    }

    /// Retrieve the scancode for a keycode, if any
    pub fn get_scancode_by_keycode(&self, keycode: u32) -> io::Result<Vec<u8>> {
        let mut keymap = input_keymap_entry {
            flags: 0,
            len: 0,
            index: 0,
            keycode,
            scancode: [0u8; 32],
        };
        unsafe { sys::eviocgkeycode_v2(self.as_raw_fd(), &mut keymap)? };
        Ok(keymap.scancode[..keymap.len as usize].to_vec())
    }

    /// Retrieve the keycode and scancode by index, starting at 0
    pub fn get_scancode_by_index(&self, index: u16) -> io::Result<(u32, Vec<u8>)> {
        let mut keymap = input_keymap_entry {
            flags: INPUT_KEYMAP_BY_INDEX,
            len: 0,
            index,
            keycode: 0,
            scancode: [0u8; 32],
        };

        unsafe { sys::eviocgkeycode_v2(self.as_raw_fd(), &mut keymap)? };
        Ok((
            keymap.keycode,
            keymap.scancode[..keymap.len as usize].to_vec(),
        ))
    }

    /// Update a scancode by index. The return value is the previous keycode
    pub fn update_scancode_by_index(
        &self,
        index: u16,
        keycode: u32,
        scancode: &[u8],
    ) -> io::Result<u32> {
        let len = scancode.len();

        let mut keymap = input_keymap_entry {
            flags: INPUT_KEYMAP_BY_INDEX,
            len: len as u8,
            index,
            keycode,
            scancode: [0u8; 32],
        };

        keymap.scancode[..len].copy_from_slice(scancode);

        let keycode = unsafe { sys::eviocskeycode_v2(self.as_raw_fd(), &keymap)? };

        Ok(keycode as u32)
    }

    /// Update a scancode. The return value is the previous keycode
    pub fn update_scancode(&self, keycode: u32, scancode: &[u8]) -> io::Result<u32> {
        let len = scancode.len();

        let mut keymap = input_keymap_entry {
            flags: 0,
            len: len as u8,
            index: 0,
            keycode,
            scancode: [0u8; 32],
        };

        keymap.scancode[..len].copy_from_slice(scancode);

        let keycode = unsafe { sys::eviocskeycode_v2(self.as_raw_fd(), &keymap)? };

        Ok(keycode as u32)
    }

    #[cfg(feature = "tokio")]
    #[inline]
    pub fn into_event_stream(self) -> io::Result<EventStream> {
        EventStream::new(self)
    }

    pub fn grab(&mut self) -> io::Result<()> {
        if !self.grabbed {
            unsafe {
                sys::eviocgrab(self.as_raw_fd(), 1)?;
            }
            self.grabbed = true;
        }
        Ok(())
    }

    pub fn ungrab(&mut self) -> io::Result<()> {
        if self.grabbed {
            unsafe {
                sys::eviocgrab(self.as_raw_fd(), 0)?;
            }
            self.grabbed = false;
        }
        Ok(())
    }

    /// Send an event to the device.
    ///
    /// Events that are typically sent to devices are
    /// [EventType::LED] (turn device LEDs on and off),
    /// [EventType::SOUND] (play a sound on the device)
    /// and [EventType::FORCEFEEDBACK] (play force feedback effects on the device, i.e. rumble).
    pub fn send_events(&mut self, events: &[InputEvent]) -> io::Result<()> {
        let bytes = unsafe { crate::cast_to_bytes(events) };
        self.file.write_all(bytes)
    }

    /// Uploads a force feedback effect to the device.
    pub fn upload_ff_effect(&mut self, data: FFEffectData) -> io::Result<FFEffect> {
        let mut effect: sys::ff_effect = data.into();
        effect.id = -1;

        unsafe { sys::eviocsff(self.file.as_raw_fd(), &effect)? };

        let file = self.file.try_clone()?;
        let id = effect.id as u16;

        Ok(FFEffect { file, id })
    }

    /// Sets the force feedback gain, i.e. how strong the force feedback effects should be for the
    /// device. A gain of 0 means no gain, whereas `u16::MAX` is the maximum gain.
    pub fn set_ff_gain(&mut self, value: u16) -> io::Result<()> {
        let events = [InputEvent::new(
            EventType::FORCEFEEDBACK,
            FFEffectType::FF_GAIN.0,
            value.into(),
        )];
        let bytes = unsafe { crate::cast_to_bytes(&events) };
        self.file.write_all(bytes)?;

        Ok(())
    }

    /// Enables or disables autocenter for the force feedback device.
    pub fn set_ff_autocenter(&mut self, value: u16) -> io::Result<()> {
        let events = [InputEvent::new(
            EventType::FORCEFEEDBACK,
            FFEffectType::FF_AUTOCENTER.0,
            value.into(),
        )];
        let bytes = unsafe { crate::cast_to_bytes(&events) };
        self.file.write_all(bytes)?;

        Ok(())
    }
}

impl AsRawFd for RawDevice {
    fn as_raw_fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }
}

/// A copy of the unstable Vec::spare_capacity_mut
#[inline]
pub(crate) fn vec_spare_capacity_mut<T>(v: &mut Vec<T>) -> &mut [mem::MaybeUninit<T>] {
    let (len, cap) = (v.len(), v.capacity());
    unsafe {
        std::slice::from_raw_parts_mut(
            v.as_mut_ptr().add(len) as *mut mem::MaybeUninit<T>,
            cap - len,
        )
    }
}

/// Crawls `/dev/input` for evdev devices.
///
/// Will not bubble up any errors in opening devices or traversing the directory. Instead returns
/// an empty iterator or omits the devices that could not be opened.
pub fn enumerate() -> EnumerateDevices {
    EnumerateDevices {
        readdir: std::fs::read_dir("/dev/input").ok(),
    }
}

pub struct EnumerateDevices {
    readdir: Option<std::fs::ReadDir>,
}
impl Iterator for EnumerateDevices {
    type Item = RawDevice;
    fn next(&mut self) -> Option<RawDevice> {
        use std::os::unix::ffi::OsStrExt;
        let readdir = self.readdir.as_mut()?;
        loop {
            if let Ok(entry) = readdir.next()? {
                let path = entry.path();
                let fname = path.file_name().unwrap();
                if fname.as_bytes().starts_with(b"event") {
                    if let Ok(dev) = RawDevice::open(&path) {
                        return Some(dev);
                    }
                }
            }
        }
    }
}

#[cfg(feature = "tokio")]
mod tokio_stream {
    use super::*;

    use tokio_1 as tokio;

    use futures_core::{ready, Stream};
    use std::pin::Pin;
    use std::task::{Context, Poll};
    use tokio::io::unix::AsyncFd;

    /// An asynchronous stream of input events.
    ///
    /// This can be used by calling [`stream.next_event().await?`](Self::next_event), or if you
    /// need to pass it as a stream somewhere, the [`futures::Stream`](Stream) implementation.
    /// There's also a lower-level [`poll_event`] function if you need to fetch an event from
    /// inside a `Future::poll` impl.
    pub struct EventStream {
        device: AsyncFd<RawDevice>,
        index: usize,
    }
    impl Unpin for EventStream {}

    impl EventStream {
        pub(crate) fn new(device: RawDevice) -> io::Result<Self> {
            use nix::fcntl;
            fcntl::fcntl(device.as_raw_fd(), fcntl::F_SETFL(fcntl::OFlag::O_NONBLOCK))?;
            let device = AsyncFd::new(device)?;
            Ok(Self { device, index: 0 })
        }

        /// Returns a reference to the underlying device
        pub fn device(&self) -> &RawDevice {
            self.device.get_ref()
        }

        /// Returns a mutable reference to the underlying device.
        pub fn device_mut(&mut self) -> &mut RawDevice {
            self.device.get_mut()
        }

        /// Try to wait for the next event in this stream. Any errors are likely to be fatal, i.e.
        /// any calls afterwards will likely error as well.
        pub async fn next_event(&mut self) -> io::Result<InputEvent> {
            poll_fn(|cx| self.poll_event(cx)).await
        }

        /// A lower-level function for directly polling this stream.
        pub fn poll_event(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<InputEvent>> {
            'outer: loop {
                if let Some(&ev) = self.device.get_ref().event_buf.get(self.index) {
                    self.index += 1;
                    return Poll::Ready(Ok(InputEvent(ev)));
                }

                self.device.get_mut().event_buf.clear();
                self.index = 0;

                loop {
                    let mut guard = ready!(self.device.poll_read_ready_mut(cx))?;

                    let res = guard.try_io(|device| device.get_mut().fill_events());
                    match res {
                        Ok(res) => {
                            let _ = res?;
                            continue 'outer;
                        }
                        Err(_would_block) => continue,
                    }
                }
            }
        }
    }

    impl Stream for EventStream {
        type Item = io::Result<InputEvent>;
        fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
            self.get_mut().poll_event(cx).map(Some)
        }
    }

    // version of futures_util::future::poll_fn
    pub(crate) fn poll_fn<T, F: FnMut(&mut Context<'_>) -> Poll<T> + Unpin>(f: F) -> PollFn<F> {
        PollFn(f)
    }
    pub(crate) struct PollFn<F>(F);
    impl<T, F: FnMut(&mut Context<'_>) -> Poll<T> + Unpin> std::future::Future for PollFn<F> {
        type Output = T;
        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
            (self.get_mut().0)(cx)
        }
    }
}
#[cfg(feature = "tokio")]
pub(crate) use tokio_stream::poll_fn;
#[cfg(feature = "tokio")]
pub use tokio_stream::EventStream;
