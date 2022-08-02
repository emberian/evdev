//! Virtual device emulation for evdev via uinput.
//!
//! This is quite useful when testing/debugging devices, or synchronization.

use crate::constants::EventType;
use crate::inputid::{BusType, InputId};
use crate::{sys, AttributeSetRef, InputEvent, Key, RelativeAxisType, SwitchType, UinputAbsSetup};
use libc::{O_NONBLOCK, uinput_abs_setup};
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::{fs::OpenOptionsExt, io::AsRawFd};
use std::path::PathBuf;

const UINPUT_PATH: &str = "/dev/uinput";
const SYSFS_PATH: &str = "/sys/devices/virtual/input";
const DEV_PATH: &str = "/dev/input";

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

    pub fn with_absolute_axis(self, axis: &UinputAbsSetup) -> io::Result<Self> {
        unsafe {
            sys::ui_set_evbit(
                self.file.as_raw_fd(),
                crate::EventType::ABSOLUTE.0 as nix::sys::ioctl::ioctl_param_type,
            )?;
            sys::ui_set_absbit(
                self.file.as_raw_fd(),
                axis.code() as nix::sys::ioctl::ioctl_param_type,
            )?;
            sys::ui_abs_setup(
                self.file.as_raw_fd(),
                &axis.0 as *const uinput_abs_setup,
            )?;
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

    /// Get the syspath representing this uinput device.
    ///
    /// The syspath returned is the one of the input node itself (e.g.
    /// `/sys/devices/virtual/input/input123`), not the syspath of the device node.
    pub fn get_syspath(&mut self) -> io::Result<PathBuf> {
        let mut bytes = vec![0u8; 256];
        unsafe { sys::ui_get_sysname(self.file.as_raw_fd(), &mut bytes)? };

        if let Some(end) = bytes.iter().position(|c| *c == 0) {
            bytes.truncate(end);
        }

        let s = String::from_utf8_lossy(&bytes).into_owned();
        let mut path = PathBuf::from(SYSFS_PATH);
        path.push(s);

        Ok(path)
    }

    /// Get the syspaths of the corresponding device nodes in /dev/input.
    pub fn enumerate_dev_nodes_blocking(&mut self) -> io::Result<DevNodesBlocking> {
        let path = self.get_syspath()?;
        let dir = std::fs::read_dir(path)?;

        Ok(DevNodesBlocking {
            dir,
        })
    }

    /// Get the syspaths of the corresponding device nodes in /dev/input.
    #[cfg(feature = "tokio")]
    pub async fn enumerate_dev_nodes(&mut self) -> io::Result<DevNodes> {
        let path = self.get_syspath()?;
        let dir = tokio_1::fs::read_dir(path).await?;

        Ok(DevNodes {
            dir,
        })
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

/// This struct is returned from the [VirtualDevice::enumerate_dev_nodes] function and will yield
/// the syspaths corresponding to the virtual device. These are of the form `/dev/input123`.
pub struct DevNodesBlocking {
    dir: std::fs::ReadDir,
}

impl Iterator for DevNodesBlocking {
    type Item = io::Result<PathBuf>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let path = self.dir.next()
                // Map the directory name to its file name.
                .map(|entry| entry.map(|entry|
                    entry.file_name().to_string_lossy().to_owned().to_string()
                ))
                // Ignore file names that do not start with "event".
                .filter(|name| name
                    .as_ref()
                    .map(|name| name.starts_with("event"))
                    .unwrap_or(true)
                )
                // Construct the path of the form `/dev/input/eventX`.
                .map(|name| name.map(|name| {
                    let mut path = PathBuf::from(DEV_PATH);
                    path.push(name);
                    path
                }));

            if let Some(value) = path {
                return Some(value);
            }
        }
    }
}

/// This struct is returned from the [VirtualDevice::enumerate_dev_nodes] function and will yield
/// the syspaths corresponding to the virtual device. These are of the form `/dev/input123`.
#[cfg(feature = "tokio")]
pub struct DevNodes {
    dir: tokio_1::fs::ReadDir,
}

#[cfg(feature = "tokio")]
impl DevNodes {
    /// Returns the next entry in the set of device nodes.
    pub async fn next_entry(&mut self) -> io::Result<Option<PathBuf>> {
        loop {
            let path = self.dir.next_entry().await?
                // Map the directory name to its file name.
                .map(|entry| entry.file_name().to_string_lossy().to_owned().to_string())
                // Ignore file names that do not start with "event".
                .filter(|name| name.starts_with("event"))
                // Construct the path of the form `/dev/input/eventX`.
                .map(|name| {
                    let mut path = PathBuf::from(DEV_PATH);
                    path.push(name);
                    path
                });

            if let Some(value) = path {
                return Ok(Some(value));
            }
        }
    }
}
