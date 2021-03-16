use std::fs::{File, OpenOptions};
use std::mem::MaybeUninit;
use std::os::unix::io::{AsRawFd, RawFd};
use std::path::Path;
use std::{io, mem};

use crate::constants::*;
use crate::{nix_err, sys, AttributeSet, AttributeSetRef, DeviceState, InputEvent, InputId, Key};

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
    id: libc::input_id,
    props: AttributeSet<PropType>,
    driver_version: (u8, u8, u8),
    supported_keys: Option<AttributeSet<Key>>,
    supported_relative: Option<AttributeSet<RelativeAxisType>>,
    supported_absolute: Option<AttributeSet<AbsoluteAxisType>>,
    supported_switch: Option<AttributeSet<SwitchType>>,
    supported_led: Option<AttributeSet<LedType>>,
    supported_misc: Option<AttributeSet<MiscType>>,
    // ff: Option<AttributeSet<_>>,
    // ff_stat: Option<FFStatus>,
    // rep: Option<Repeat>,
    supported_snd: Option<AttributeSet<SoundType>>,
    pub(crate) event_buf: Vec<libc::input_event>,
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
            unsafe {
                sys::eviocgbit_type(file.as_raw_fd(), ty.as_mut_raw_slice()).map_err(nix_err)?
            };
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
            sys::eviocgid(file.as_raw_fd(), id.as_mut_ptr()).map_err(nix_err)?;
            id.assume_init()
        };
        let mut driver_version: i32 = 0;
        unsafe {
            sys::eviocgversion(file.as_raw_fd(), &mut driver_version).map_err(nix_err)?;
        }
        let driver_version = (
            ((driver_version >> 16) & 0xff) as u8,
            ((driver_version >> 8) & 0xff) as u8,
            (driver_version & 0xff) as u8,
        );

        let props = {
            let mut props = AttributeSet::<PropType>::new();
            unsafe {
                sys::eviocgprop(file.as_raw_fd(), props.as_mut_raw_slice()).map_err(nix_err)?
            };
            props
        }; // FIXME: handle old kernel

        let supported_keys = if ty.contains(EventType::KEY) {
            let mut keys = AttributeSet::<Key>::new();
            unsafe {
                sys::eviocgbit_key(file.as_raw_fd(), keys.as_mut_raw_slice()).map_err(nix_err)?
            };
            Some(keys)
        } else {
            None
        };

        let supported_relative = if ty.contains(EventType::RELATIVE) {
            let mut rel = AttributeSet::<RelativeAxisType>::new();
            unsafe {
                sys::eviocgbit_relative(file.as_raw_fd(), rel.as_mut_raw_slice())
                    .map_err(nix_err)?
            };
            Some(rel)
        } else {
            None
        };

        let supported_absolute = if ty.contains(EventType::ABSOLUTE) {
            let mut abs = AttributeSet::<AbsoluteAxisType>::new();
            unsafe {
                sys::eviocgbit_absolute(file.as_raw_fd(), abs.as_mut_raw_slice())
                    .map_err(nix_err)?
            };
            Some(abs)
        } else {
            None
        };

        let supported_switch = if ty.contains(EventType::SWITCH) {
            let mut switch = AttributeSet::<SwitchType>::new();
            unsafe {
                sys::eviocgbit_switch(file.as_raw_fd(), switch.as_mut_raw_slice())
                    .map_err(nix_err)?
            };
            Some(switch)
        } else {
            None
        };

        let supported_led = if ty.contains(EventType::LED) {
            let mut led = AttributeSet::<LedType>::new();
            unsafe {
                sys::eviocgbit_led(file.as_raw_fd(), led.as_mut_raw_slice()).map_err(nix_err)?
            };
            Some(led)
        } else {
            None
        };

        let supported_misc = if ty.contains(EventType::MISC) {
            let mut misc = AttributeSet::<MiscType>::new();
            unsafe {
                sys::eviocgbit_misc(file.as_raw_fd(), misc.as_mut_raw_slice()).map_err(nix_err)?
            };
            Some(misc)
        } else {
            None
        };

        //unsafe { sys::eviocgbit(file.as_raw_fd(), ffs(FORCEFEEDBACK.bits()), 0x7f, bits_as_u8_slice)?; }

        let supported_snd = if ty.contains(EventType::SOUND) {
            let mut snd = AttributeSet::<SoundType>::new();
            unsafe {
                sys::eviocgbit_sound(file.as_raw_fd(), snd.as_mut_raw_slice()).map_err(nix_err)?
            };
            Some(snd)
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
            supported_snd,
            event_buf: Vec::new(),
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
    /// ```
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
    /// ```
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
    /// ```
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
    /// ```
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

    // pub fn supported_repeats(&self) -> Option<Repeat> {
    //     self.rep
    // }

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
        let (_, uninit_buf, _) = unsafe { spare_capacity.align_to_mut::<mem::MaybeUninit<u8>>() };

        // use libc::read instead of nix::unistd::read b/c we need to pass an uninitialized buf
        let res = unsafe { libc::read(fd, uninit_buf.as_mut_ptr() as _, uninit_buf.len()) };
        let bytes_read = nix::errno::Errno::result(res).map_err(nix_err)?;
        let num_read = bytes_read as usize / mem::size_of::<libc::input_event>();
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

    /// Create an empty `DeviceState`. The `{abs,key,etc}_vals` for the returned state will return
    /// `Some` if `self.supported_events()` contains that `EventType`.
    pub fn empty_state(&self) -> DeviceState {
        let supports = self.supported_events();

        let key_vals = if supports.contains(EventType::KEY) {
            Some(AttributeSet::new())
        } else {
            None
        };
        let abs_vals = if supports.contains(EventType::ABSOLUTE) {
            #[rustfmt::skip]
            const ABSINFO_ZERO: libc::input_absinfo = libc::input_absinfo {
                value: 0, minimum: 0, maximum: 0, fuzz: 0, flat: 0, resolution: 0,
            };
            const ABS_VALS_INIT: [libc::input_absinfo; AbsoluteAxisType::COUNT] =
                [ABSINFO_ZERO; AbsoluteAxisType::COUNT];
            Some(Box::new(ABS_VALS_INIT))
        } else {
            None
        };
        let switch_vals = if supports.contains(EventType::SWITCH) {
            Some(AttributeSet::new())
        } else {
            None
        };
        let led_vals = if supports.contains(EventType::LED) {
            Some(AttributeSet::new())
        } else {
            None
        };

        DeviceState {
            timestamp: libc::timeval {
                tv_sec: 0,
                tv_usec: 0,
            },
            key_vals,
            abs_vals,
            switch_vals,
            led_vals,
        }
    }

    pub fn sync_state(&self, state: &mut DeviceState) -> io::Result<()> {
        self.sync_key_state(state)?;
        self.sync_abs_state(state)?;
        self.sync_switch_state(state)?;
        self.sync_led_state(state)?;
        Ok(())
    }

    pub fn sync_key_state(&self, state: &mut DeviceState) -> io::Result<()> {
        if let Some(key_vals) = &mut state.key_vals {
            unsafe {
                sys::eviocgkey(self.as_raw_fd(), key_vals.as_mut_raw_slice()).map_err(nix_err)?
            };
        }
        Ok(())
    }

    pub fn sync_abs_state(&self, state: &mut DeviceState) -> io::Result<()> {
        if let (Some(supported_abs), Some(abs_vals)) =
            (self.supported_absolute_axes(), &mut state.abs_vals)
        {
            for AbsoluteAxisType(idx) in supported_abs.iter() {
                // ignore multitouch, we'll handle that later.
                //
                // handling later removed. not sure what the intention of "handling that later" was
                // the abs data seems to be fine (tested ABS_MT_POSITION_X/Y)
                unsafe {
                    sys::eviocgabs(self.as_raw_fd(), idx as u32, &mut abs_vals[idx as usize])
                        .map_err(nix_err)?
                };
            }
        }
        Ok(())
    }

    pub fn sync_switch_state(&self, state: &mut DeviceState) -> io::Result<()> {
        if let Some(switch_vals) = &mut state.switch_vals {
            unsafe {
                sys::eviocgsw(self.as_raw_fd(), switch_vals.as_mut_raw_slice()).map_err(nix_err)?
            };
        }
        Ok(())
    }

    pub fn sync_led_state(&self, state: &mut DeviceState) -> io::Result<()> {
        if let Some(led_vals) = &mut state.led_vals {
            unsafe {
                sys::eviocgled(self.as_raw_fd(), led_vals.as_mut_raw_slice()).map_err(nix_err)?
            };
        }
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
fn vec_spare_capacity_mut<T>(v: &mut Vec<T>) -> &mut [mem::MaybeUninit<T>] {
    let (len, cap) = (v.len(), v.capacity());
    unsafe {
        std::slice::from_raw_parts_mut(
            v.as_mut_ptr().add(len) as *mut mem::MaybeUninit<T>,
            cap - len,
        )
    }
}
