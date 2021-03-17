//! Virtual device emulation for evdev via uinput.
//!
//! This is quite useful when testing/debugging devices, or synchronization.

use crate::constants::EventType;
use crate::{nix_err, sys, AttributeSetRef, InputEvent, Key, RelativeAxisType};
use libc::O_NONBLOCK;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::{fs::OpenOptionsExt, io::AsRawFd};

const UINPUT_MAX_NAME_SIZE: usize = 80;
const BUS_USB: u16 = 0x03;
const UINPUT_PATH: &str = "/dev/uinput";

#[repr(C)]
#[derive(Debug)]
pub struct uinput_setup {
    pub id: libc::input_id,
    pub name: [u8; UINPUT_MAX_NAME_SIZE],
    pub ff_effects_max: u32,
}

#[derive(Debug)]
pub struct VirtualDeviceBuilder<'a> {
    file: File,
    name: &'a str,
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

    pub fn name(mut self, name: &'a str) -> Self {
        self.name = name;
        self
    }

    pub fn input_id(mut self, id: libc::input_id) -> Self {
        self.id = Some(id);
        self
    }

    pub fn with_keys(self, keys: &AttributeSetRef<Key>) -> io::Result<Self> {
        // Run ioctls for setting capability bits
        unsafe {
            sys::ui_set_evbit(
                self.file.as_raw_fd(),
                crate::EventType::KEY.0 as nix::sys::ioctl::ioctl_param_type,
            )
        }
        .map_err(nix_err)?;

        for bit in keys.iter() {
            unsafe {
                sys::ui_set_keybit(
                    self.file.as_raw_fd(),
                    bit.0 as nix::sys::ioctl::ioctl_param_type,
                )
            }
            .map_err(nix_err)?;
        }

        Ok(self)
    }

    pub fn with_relative_axes(self, axes: &AttributeSetRef<RelativeAxisType>) -> io::Result<Self> {
        unsafe {
            sys::ui_set_evbit(
                self.file.as_raw_fd(),
                crate::EventType::RELATIVE.0 as nix::sys::ioctl::ioctl_param_type,
            )
        }
        .map_err(nix_err)?;

        for bit in axes.iter() {
            unsafe {
                sys::ui_set_relbit(
                    self.file.as_raw_fd(),
                    bit.0 as nix::sys::ioctl::ioctl_param_type,
                )
            }
            .map_err(nix_err)?;
        }

        Ok(self)
    }

    pub fn build(self) -> io::Result<VirtualDevice> {
        // Populate the uinput_setup struct

        let mut usetup = uinput_setup {
            id: self.id.unwrap_or(DEFAULT_ID),
            name: [0u8; UINPUT_MAX_NAME_SIZE],
            ff_effects_max: 0,
        };

        let name_bytes = self.name.as_bytes();
        // Panic if we're doing something really stupid
        // + 1 for the null terminator; usetup.name was zero-initialized so there will be null
        // bytes after the part we copy into
        assert!(name_bytes.len() + 1 < UINPUT_MAX_NAME_SIZE);
        usetup.name[..name_bytes.len()].copy_from_slice(name_bytes);

        VirtualDevice::new(self.file, &usetup)
    }
}

const DEFAULT_ID: libc::input_id = libc::input_id {
    bustype: BUS_USB,
    vendor: 0x1234,  /* sample vendor */
    product: 0x5678, /* sample product */
    version: 0x111,
};

pub struct VirtualDevice {
    file: File,
}

impl VirtualDevice {
    /// Create a new virtual device.
    fn new(file: File, usetup: &uinput_setup) -> io::Result<Self> {
        unsafe { sys::ui_dev_setup(file.as_raw_fd(), usetup) }.map_err(nix_err)?;
        unsafe { sys::ui_dev_create(file.as_raw_fd()) }.map_err(nix_err)?;

        Ok(VirtualDevice { file })
    }

    #[inline]
    fn write_raw(&mut self, messages: &[InputEvent]) -> io::Result<()> {
        let bytes = unsafe { crate::cast_to_bytes(messages) };
        self.file.write_all(bytes)
    }

    /// Post a set of messages to the virtual device.
    ///
    /// This inserts a SYN_REPORT for you, because apparently uinput requires that for the
    /// kernel to realize we're done.
    pub fn emit(&mut self, messages: &[InputEvent]) -> io::Result<()> {
        self.write_raw(messages)?;

        // Now we have to write a SYN_REPORT as well.
        let syn = InputEvent::new(EventType::SYNCHRONIZATION, 0, 0);
        self.write_raw(&[syn])?;

        Ok(())
    }
}
