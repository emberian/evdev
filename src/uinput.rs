//! Virtual device emulation for evdev via uinput.
//!
//! This is quite useful when testing/debugging devices, or synchronization.

use crate::{nix_err, sys, AttributeSetRef, InputEvent, Key, RelativeAxisType};
use libc::O_NONBLOCK;
use std::io::{self, Write};
use std::os::unix::{fs::OpenOptionsExt, io::AsRawFd};
use std::slice::from_raw_parts;
use std::{
    ffi::CString,
    fs::{File, OpenOptions},
};

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
        let c_name = CString::new(self.name).unwrap();
        let c_name_bytes = c_name.as_bytes_with_nul();
        // Panic if we're doing something really stupid
        assert!(c_name_bytes.len() < UINPUT_MAX_NAME_SIZE);
        let mut name: [u8; UINPUT_MAX_NAME_SIZE] = unsafe { std::mem::zeroed() };
        name[..c_name_bytes.len()].copy_from_slice(c_name_bytes);

        let usetup = uinput_setup {
            id: self.id.unwrap_or(libc::input_id {
                bustype: BUS_USB,
                vendor: 0x1234,  /* sample vendor */
                product: 0x5678, /* sample product */
                version: 0x111,
            }),
            name,
            ff_effects_max: 0,
        };

        VirtualDevice::new(self.file, usetup)
    }
}

pub struct VirtualDevice {
    file: File,
}

impl VirtualDevice {
    /// Create a new virtual device.
    fn new(file: File, usetup: uinput_setup) -> io::Result<Self> {
        unsafe { sys::ui_dev_setup(file.as_raw_fd(), &usetup) }.map_err(nix_err)?;
        unsafe { sys::ui_dev_create(file.as_raw_fd()) }.map_err(nix_err)?;

        Ok(VirtualDevice { file })
    }

    /// Post a set of messages to the virtual device.
    ///
    /// This inserts a SYN_REPORT for you, because apparently uinput requires that for the
    /// kernel to realize we're done.
    pub fn emit(&mut self, messages: &[InputEvent]) -> std::io::Result<usize> {
        let messages: &[u8] = unsafe {
            from_raw_parts(
                messages as *const _ as *const u8,
                messages.len() * std::mem::size_of::<InputEvent>(),
            )
        };
        let written = self.file.write(messages)?;

        // Now we have to write a SYN_REPORT as well.
        let syn = InputEvent::new(0, 0, 0);
        let syn_bytes = unsafe {
            from_raw_parts(
                &syn as *const _ as *const u8,
                std::mem::size_of::<InputEvent>(),
            )
        };
        let _ = self.file.write(syn_bytes)?;

        Ok(written)
    }
}
