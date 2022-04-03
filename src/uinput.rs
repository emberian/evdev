//! Virtual device emulation for evdev via uinput.
//!
//! This is quite useful when testing/debugging devices, or synchronization.

use crate::constants::EventType;
use crate::inputid::{BusType, InputId};
use crate::{sys, AttributeSetRef, InputEvent, Key, RelativeAxisType, SwitchType};
use libc::O_NONBLOCK;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::{fs::OpenOptionsExt, io::AsRawFd};

const UINPUT_PATH: &str = "/dev/uinput";

#[derive(Debug)]
pub struct VirtualDeviceBuilder<'a> {
    file: File,
    name: &'a [u8],
    id: Option<libc::input_id>,
}

impl<'a> VirtualDeviceBuilder<'a> {
    pub fn new() -> io::Result<Self> {
        let mut options = OpenOptions::new();

        // Open in write-only, in nonblocking mode.
        let file = options
            .write(true)
            .custom_flags(O_NONBLOCK)
            .open(UINPUT_PATH)?;

        Ok(VirtualDeviceBuilder {
            file,
            name: Default::default(),
            id: None,
        })
    }

    #[inline]
    pub fn name<S: AsRef<[u8]> + ?Sized>(mut self, name: &'a S) -> Self {
        self.name = name.as_ref();
        self
    }

    #[inline]
    pub fn input_id(mut self, id: InputId) -> Self {
        self.id = Some(id.0);
        self
    }

    pub fn with_keys(self, keys: &AttributeSetRef<Key>) -> io::Result<Self> {
        // Run ioctls for setting capability bits
        unsafe {
            sys::ui_set_evbit(
                self.file.as_raw_fd(),
                crate::EventType::KEY.0 as nix::sys::ioctl::ioctl_param_type,
            )?;
        }

        for bit in keys.iter() {
            unsafe {
                sys::ui_set_keybit(
                    self.file.as_raw_fd(),
                    bit.0 as nix::sys::ioctl::ioctl_param_type,
                )?;
            }
        }

        Ok(self)
    }

    pub fn with_relative_axes(self, axes: &AttributeSetRef<RelativeAxisType>) -> io::Result<Self> {
        unsafe {
            sys::ui_set_evbit(
                self.file.as_raw_fd(),
                crate::EventType::RELATIVE.0 as nix::sys::ioctl::ioctl_param_type,
            )?;
        }

        for bit in axes.iter() {
            unsafe {
                sys::ui_set_relbit(
                    self.file.as_raw_fd(),
                    bit.0 as nix::sys::ioctl::ioctl_param_type,
                )?;
            }
        }

        Ok(self)
    }

    pub fn with_switches(self, switches: &AttributeSetRef<SwitchType>) -> io::Result<Self> {
        unsafe {
            sys::ui_set_evbit(
                self.file.as_raw_fd(),
                crate::EventType::SWITCH.0 as nix::sys::ioctl::ioctl_param_type,
            )?;
        }

        for bit in switches.iter() {
            unsafe {
                sys::ui_set_swbit(
                    self.file.as_raw_fd(),
                    bit.0 as nix::sys::ioctl::ioctl_param_type,
                )?;
            }
        }

        Ok(self)
    }

    pub fn build(self) -> io::Result<VirtualDevice> {
        // Populate the uinput_setup struct

        let mut usetup = libc::uinput_setup {
            id: self.id.unwrap_or(DEFAULT_ID),
            name: [0; libc::UINPUT_MAX_NAME_SIZE],
            ff_effects_max: 0,
        };

        // SAFETY: either casting [u8] to [u8], or [u8] to [i8], which is the same size
        let name_bytes = unsafe { &*(self.name as *const [u8] as *const [libc::c_char]) };
        // Panic if we're doing something really stupid
        // + 1 for the null terminator; usetup.name was zero-initialized so there will be null
        // bytes after the part we copy into
        assert!(name_bytes.len() + 1 < libc::UINPUT_MAX_NAME_SIZE);
        usetup.name[..name_bytes.len()].copy_from_slice(name_bytes);

        VirtualDevice::new(self.file, &usetup)
    }
}

const DEFAULT_ID: libc::input_id = libc::input_id {
    bustype: BusType::BUS_USB.0,
    vendor: 0x1234,  /* sample vendor */
    product: 0x5678, /* sample product */
    version: 0x111,
};

pub struct VirtualDevice {
    file: File,
}

impl VirtualDevice {
    /// Create a new virtual device.
    fn new(file: File, usetup: &libc::uinput_setup) -> io::Result<Self> {
        unsafe { sys::ui_dev_setup(file.as_raw_fd(), usetup)? };
        unsafe { sys::ui_dev_create(file.as_raw_fd())? };

        Ok(VirtualDevice { file })
    }

    #[inline]
    fn write_raw(&mut self, messages: &[InputEvent]) -> io::Result<()> {
        let bytes = unsafe { crate::cast_to_bytes(messages) };
        self.file.write_all(bytes)
    }

    /// Post a batch of events to the virtual device.
    ///
    /// The batch is automatically terminated with a `SYN_REPORT` event.
    /// Events from physical devices are batched based on if they occur simultaneously, for example movement
    /// of a mouse triggers a movement events for the X and Y axes separately in a batch of 2 events.
    ///
    /// Single events such as a `KEY` event must still be followed by a `SYN_REPORT`.
    pub fn emit(&mut self, messages: &[InputEvent]) -> io::Result<()> {
        self.write_raw(messages)?;
        let syn = InputEvent::new(EventType::SYNCHRONIZATION, 0, 0);
        self.write_raw(&[syn])
    }
}
