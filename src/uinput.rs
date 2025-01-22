//! Virtual device emulation for evdev via uinput.
//!
//! This is quite useful when testing/debugging devices, or synchronization.

use crate::compat::{input_event, input_id, uinput_abs_setup, uinput_setup, UINPUT_MAX_NAME_SIZE};
use crate::ff::FFEffectData;
use crate::inputid::{BusType, InputId};
use crate::{
    sys, AttributeSetRef, FFEffectCode, InputEvent, KeyCode, MiscCode, PropType, RelativeAxisCode,
    SwitchCode, SynchronizationEvent, UInputCode, UInputEvent, UinputAbsSetup,
};
use std::ffi::{CStr, OsStr};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::{fs, io};

const UINPUT_PATH: &str = "/dev/uinput";
const SYSFS_PATH: &str = "/sys/devices/virtual/input";
const DEV_PATH: &str = "/dev/input";

/// A builder struct for creating a new uinput virtual device.
#[derive(Debug)]
pub struct VirtualDeviceBuilder<'a> {
    fd: OwnedFd,
    name: &'a [u8],
    id: Option<input_id>,
    ff_effects_max: u32,
}

/// A builder struct for [`VirtualDevice`].
///
/// Created via [`VirtualDevice::builder()`].
impl<'a> VirtualDeviceBuilder<'a> {
    #[deprecated(note = "use `VirtualDevice::builder()` instead")]
    #[doc(hidden)]
    pub fn new() -> io::Result<Self> {
        // Open in read-write mode.
        let fd = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(UINPUT_PATH)?;

        Ok(VirtualDeviceBuilder {
            fd: fd.into(),
            name: Default::default(),
            id: None,
            ff_effects_max: 0,
        })
    }

    /// Set the display name of this device.
    #[inline]
    pub fn name<S: AsRef<[u8]> + ?Sized>(mut self, name: &'a S) -> Self {
        self.name = name.as_ref();
        self
    }

    /// Set a custom input ID.
    #[inline]
    pub fn input_id(mut self, id: InputId) -> Self {
        self.id = Some(id.0);
        self
    }

    /// Set the device's physical location, e.g. `usb-00:01.2-2.1/input0`.
    pub fn with_phys(self, path: &CStr) -> io::Result<Self> {
        unsafe {
            sys::ui_set_phys(self.fd.as_raw_fd(), path.as_ptr())?;
        }
        Ok(self)
    }

    /// Set the key codes that can be emitted by this device.
    pub fn with_keys(self, keys: &AttributeSetRef<KeyCode>) -> io::Result<Self> {
        // Run ioctls for setting capability bits
        unsafe {
            sys::ui_set_evbit(
                self.fd.as_raw_fd(),
                crate::EventType::KEY.0 as nix::sys::ioctl::ioctl_param_type,
            )?;
        }

        for bit in keys.iter() {
            unsafe {
                sys::ui_set_keybit(
                    self.fd.as_raw_fd(),
                    bit.0 as nix::sys::ioctl::ioctl_param_type,
                )?;
            }
        }

        Ok(self)
    }

    /// Set the absolute axes of this device.
    pub fn with_absolute_axis(self, axis: &UinputAbsSetup) -> io::Result<Self> {
        unsafe {
            sys::ui_set_evbit(
                self.fd.as_raw_fd(),
                crate::EventType::ABSOLUTE.0 as nix::sys::ioctl::ioctl_param_type,
            )?;
            sys::ui_set_absbit(
                self.fd.as_raw_fd(),
                axis.code() as nix::sys::ioctl::ioctl_param_type,
            )?;
            sys::ui_abs_setup(self.fd.as_raw_fd(), &axis.0 as *const uinput_abs_setup)?;
        }

        Ok(self)
    }

    /// Set the relative axes of this device.
    pub fn with_relative_axes(self, axes: &AttributeSetRef<RelativeAxisCode>) -> io::Result<Self> {
        unsafe {
            sys::ui_set_evbit(
                self.fd.as_raw_fd(),
                crate::EventType::RELATIVE.0 as nix::sys::ioctl::ioctl_param_type,
            )?;
        }

        for bit in axes.iter() {
            unsafe {
                sys::ui_set_relbit(
                    self.fd.as_raw_fd(),
                    bit.0 as nix::sys::ioctl::ioctl_param_type,
                )?;
            }
        }

        Ok(self)
    }

    /// Set the properties of this device.
    pub fn with_properties(self, switches: &AttributeSetRef<PropType>) -> io::Result<Self> {
        for bit in switches.iter() {
            unsafe {
                sys::ui_set_propbit(
                    self.fd.as_raw_fd(),
                    bit.0 as nix::sys::ioctl::ioctl_param_type,
                )?;
            }
        }

        Ok(self)
    }

    /// Set the switch codes that can be emitted by this device.
    pub fn with_switches(self, switches: &AttributeSetRef<SwitchCode>) -> io::Result<Self> {
        unsafe {
            sys::ui_set_evbit(
                self.fd.as_raw_fd(),
                crate::EventType::SWITCH.0 as nix::sys::ioctl::ioctl_param_type,
            )?;
        }

        for bit in switches.iter() {
            unsafe {
                sys::ui_set_swbit(
                    self.fd.as_raw_fd(),
                    bit.0 as nix::sys::ioctl::ioctl_param_type,
                )?;
            }
        }

        Ok(self)
    }

    /// Set the force-feedback effects that can be emitted by this device.
    pub fn with_ff(self, ff: &AttributeSetRef<FFEffectCode>) -> io::Result<Self> {
        unsafe {
            sys::ui_set_evbit(
                self.fd.as_raw_fd(),
                crate::EventType::FORCEFEEDBACK.0 as nix::sys::ioctl::ioctl_param_type,
            )?;
        }

        for bit in ff.iter() {
            unsafe {
                sys::ui_set_ffbit(
                    self.fd.as_raw_fd(),
                    bit.0 as nix::sys::ioctl::ioctl_param_type,
                )?;
            }
        }

        Ok(self)
    }

    /// Set the maximum number for a force-feedback effect for this device.
    pub fn with_ff_effects_max(mut self, ff_effects_max: u32) -> Self {
        self.ff_effects_max = ff_effects_max;
        self
    }

    /// Set the `MiscCode`s of this device.
    pub fn with_msc(self, misc_set: &AttributeSetRef<MiscCode>) -> io::Result<Self> {
        unsafe {
            sys::ui_set_evbit(
                self.fd.as_raw_fd(),
                crate::EventType::MISC.0 as nix::sys::ioctl::ioctl_param_type,
            )?;
        }

        for bit in misc_set.iter() {
            unsafe {
                sys::ui_set_mscbit(
                    self.fd.as_raw_fd(),
                    bit.0 as nix::sys::ioctl::ioctl_param_type,
                )?;
            }
        }

        Ok(self)
    }

    /// Finalize and register this device.
    ///
    /// # Errors
    /// Returns an error if device setup or creation fails.
    pub fn build(self) -> io::Result<VirtualDevice> {
        // Populate the uinput_setup struct

        let mut usetup = uinput_setup {
            id: self.id.unwrap_or(DEFAULT_ID),
            name: [0; UINPUT_MAX_NAME_SIZE],
            ff_effects_max: self.ff_effects_max,
        };

        // SAFETY: either casting [u8] to [u8], or [u8] to [i8], which is the same size
        let name_bytes = unsafe { &*(self.name as *const [u8] as *const [libc::c_char]) };
        // Panic if we're doing something really stupid
        // + 1 for the null terminator; usetup.name was zero-initialized so there will be null
        // bytes after the part we copy into
        assert!(name_bytes.len() + 1 < UINPUT_MAX_NAME_SIZE);
        usetup.name[..name_bytes.len()].copy_from_slice(name_bytes);

        VirtualDevice::new(self.fd, &usetup)
    }
}

const DEFAULT_ID: input_id = input_id {
    bustype: BusType::BUS_USB.0,
    vendor: 0x1234,  /* sample vendor */
    product: 0x5678, /* sample product */
    version: 0x111,
};

/// A handle to a uinput virtual device.
#[derive(Debug)]
pub struct VirtualDevice {
    fd: OwnedFd,
    pub(crate) event_buf: Vec<input_event>,
}

impl VirtualDevice {
    /// Convenience method for creating a `VirtualDeviceBuilder`.
    pub fn builder<'a>() -> io::Result<VirtualDeviceBuilder<'a>> {
        #[allow(deprecated)]
        VirtualDeviceBuilder::new()
    }

    /// Create a new virtual device.
    fn new(fd: OwnedFd, usetup: &uinput_setup) -> io::Result<Self> {
        unsafe { sys::ui_dev_setup(fd.as_raw_fd(), usetup)? };
        unsafe { sys::ui_dev_create(fd.as_raw_fd())? };

        Ok(VirtualDevice {
            fd,
            event_buf: vec![],
        })
    }

    #[inline]
    fn write_raw(&mut self, events: &[InputEvent]) -> io::Result<()> {
        crate::write_events(self.fd.as_fd(), events)?;
        Ok(())
    }

    /// Get the syspath representing this uinput device.
    ///
    /// The syspath returned is the one of the input node itself (e.g.
    /// `/sys/devices/virtual/input/input123`), not the syspath of the device node.
    pub fn get_syspath(&mut self) -> io::Result<PathBuf> {
        let mut syspath = vec![0u8; 256];
        let len = unsafe { sys::ui_get_sysname(self.fd.as_raw_fd(), &mut syspath)? };
        syspath.truncate(len as usize - 1);

        let syspath = OsStr::from_bytes(&syspath);

        Ok(Path::new(SYSFS_PATH).join(syspath))
    }

    /// Get the syspaths of the corresponding device nodes in /dev/input.
    pub fn enumerate_dev_nodes_blocking(&mut self) -> io::Result<DevNodesBlocking> {
        let path = self.get_syspath()?;
        let dir = std::fs::read_dir(path)?;

        Ok(DevNodesBlocking { dir })
    }

    /// Get the syspaths of the corresponding device nodes in /dev/input.
    #[cfg(feature = "tokio")]
    pub async fn enumerate_dev_nodes(&mut self) -> io::Result<DevNodes> {
        let path = self.get_syspath()?;
        let dir = tokio::fs::read_dir(path).await?;

        Ok(DevNodes { dir })
    }

    /// Post a batch of events to the virtual device.
    ///
    /// The batch is automatically terminated with a `SYN_REPORT` event.
    /// Events from physical devices are batched based on if they occur simultaneously, for example movement
    /// of a mouse triggers a movement events for the X and Y axes separately in a batch of 2 events.
    ///
    /// Single events such as a `KEY` event must still be followed by a `SYN_REPORT`.
    pub fn emit(&mut self, events: &[InputEvent]) -> io::Result<()> {
        self.write_raw(events)?;
        let syn = *SynchronizationEvent::new(crate::SynchronizationCode::SYN_REPORT, 0);
        self.write_raw(&[syn])
    }

    /// Processes the given [`UInputEvent`] if it is a force feedback upload event, in which case
    /// this function will start the force feedback upload and claim ownership over the
    /// [`UInputEvent`] and return a [`FFUploadEvent`] instead.
    ///
    /// The returned event allows the user to allocate and set the effect ID as well as access the
    /// effect data.
    ///
    /// # Panics
    ///
    /// This function will panic if `event.code()` is not `UI_FF_UPLOAD`.
    pub fn process_ff_upload(&mut self, event: UInputEvent) -> io::Result<FFUploadEvent> {
        assert_eq!(event.code(), UInputCode::UI_FF_UPLOAD);

        let mut request: sys::uinput_ff_upload = unsafe { std::mem::zeroed() };
        request.request_id = event.value() as u32;
        unsafe { sys::ui_begin_ff_upload(self.fd.as_raw_fd(), &mut request)? };

        request.retval = 0;

        let fd = self.fd.try_clone()?;

        Ok(FFUploadEvent { fd, request })
    }

    /// Processes the given [`UInputEvent`] if it is a force feedback erase event, in which case
    /// this function will start the force feedback erasure and claim ownership over the
    /// [`UInputEvent`] and return a [`FFEraseEvent`] instead.
    ///
    /// The returned event allows the user to access the effect ID, such that it can free any
    /// memory used for the given effect ID.
    ///
    /// # Panics
    ///
    /// This function will panic if `event.code()` is not `UI_FF_ERASE`.
    pub fn process_ff_erase(&mut self, event: UInputEvent) -> io::Result<FFEraseEvent> {
        assert_eq!(event.code(), UInputCode::UI_FF_ERASE);

        let mut request: sys::uinput_ff_erase = unsafe { std::mem::zeroed() };
        request.request_id = event.value() as u32;
        unsafe { sys::ui_begin_ff_erase(self.fd.as_raw_fd(), &mut request)? };

        request.retval = 0;

        let fd = self.fd.try_clone()?;

        Ok(FFEraseEvent { fd, request })
    }

    /// Read a maximum of `num` events into the internal buffer. If the underlying fd is not
    /// O_NONBLOCK, this will block.
    ///
    /// Returns the number of events that were read, or an error.
    pub(crate) fn fill_events(&mut self) -> io::Result<usize> {
        let fd = self.fd.as_raw_fd();
        self.event_buf.reserve(crate::EVENT_BATCH_SIZE);

        let spare_capacity = self.event_buf.spare_capacity_mut();
        let spare_capacity_size = std::mem::size_of_val(spare_capacity);

        // use libc::read instead of nix::unistd::read b/c we need to pass an uninitialized buf
        let res = unsafe { libc::read(fd, spare_capacity.as_mut_ptr() as _, spare_capacity_size) };
        let bytes_read = nix::errno::Errno::result(res)?;
        let num_read = bytes_read as usize / std::mem::size_of::<input_event>();
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
        Ok(self.event_buf.drain(..).map(InputEvent::from))
    }

    #[cfg(feature = "tokio")]
    #[inline]
    pub fn into_event_stream(self) -> io::Result<VirtualEventStream> {
        VirtualEventStream::new(self)
    }
}

/// This struct is returned from the [VirtualDevice::enumerate_dev_nodes_blocking] function and will yield
/// the syspaths corresponding to the virtual device. These are of the form `/dev/input123`.
pub struct DevNodesBlocking {
    dir: std::fs::ReadDir,
}

impl Iterator for DevNodesBlocking {
    type Item = io::Result<PathBuf>;

    fn next(&mut self) -> Option<Self::Item> {
        for entry in self.dir.by_ref() {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => return Some(Err(e)),
            };

            // Map the directory name to its file name.
            let file_name = entry.file_name();

            // Ignore file names that do not start with event.
            if !file_name.as_bytes().starts_with(b"event") {
                continue;
            }

            // Construct the path of the form '/dev/input/eventX'.
            let path = Path::new(DEV_PATH).join(file_name);

            return Some(Ok(path));
        }

        None
    }
}

/// This struct is returned from the [VirtualDevice::enumerate_dev_nodes_blocking] function and
/// will yield the syspaths corresponding to the virtual device. These are of the form
/// `/dev/input123`.
#[cfg(feature = "tokio")]
pub struct DevNodes {
    dir: tokio::fs::ReadDir,
}

#[cfg(feature = "tokio")]
impl DevNodes {
    /// Returns the next entry in the set of device nodes.
    pub async fn next_entry(&mut self) -> io::Result<Option<PathBuf>> {
        while let Some(entry) = self.dir.next_entry().await? {
            // Map the directory name to its file name.
            let file_name = entry.file_name();

            // Ignore file names that do not start with event.
            if !file_name.as_bytes().starts_with(b"event") {
                continue;
            }

            // Construct the path of the form '/dev/input/eventX'.
            let path = Path::new(DEV_PATH).join(file_name);

            return Ok(Some(path));
        }

        Ok(None)
    }
}

impl AsFd for VirtualDevice {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}

impl AsRawFd for VirtualDevice {
    fn as_raw_fd(&self) -> RawFd {
        self.fd.as_raw_fd()
    }
}

/// Represents a force feedback upload event that we are currently processing.
pub struct FFUploadEvent {
    fd: OwnedFd,
    request: sys::uinput_ff_upload,
}

impl FFUploadEvent {
    /// Returns the old effect data.
    pub fn old_effect(&self) -> FFEffectData {
        self.request.old.into()
    }

    /// Returns the new effect ID.
    pub fn effect_id(&self) -> i16 {
        self.request.effect.id
    }

    /// Sets the new effect ID.
    pub fn set_effect_id(&mut self, id: i16) {
        self.request.effect.id = id;
    }

    /// Returns the new effect data.
    pub fn effect(&self) -> FFEffectData {
        self.request.effect.into()
    }

    /// Returns the currently set return value for the upload event.
    pub fn retval(&self) -> i32 {
        self.request.retval
    }

    /// Sets the return value to return for the upload event.
    pub fn set_retval(&mut self, value: i32) {
        self.request.retval = value;
    }
}

impl Drop for FFUploadEvent {
    fn drop(&mut self) {
        unsafe {
            let _ = sys::ui_end_ff_upload(self.fd.as_raw_fd(), &self.request);
        }
    }
}

/// Represents a force feedback erase event that we are currently processing.
pub struct FFEraseEvent {
    fd: OwnedFd,
    request: sys::uinput_ff_erase,
}

impl FFEraseEvent {
    /// Returns the effect ID to erase.
    pub fn effect_id(&self) -> u32 {
        self.request.effect_id
    }

    /// Returns the currently set return value for the erase event.
    pub fn retval(&self) -> i32 {
        self.request.retval
    }

    /// Sets the return value to return for the erase event.
    pub fn set_retval(&mut self, value: i32) {
        self.request.retval = value;
    }
}

impl Drop for FFEraseEvent {
    fn drop(&mut self) {
        unsafe {
            let _ = sys::ui_end_ff_erase(self.fd.as_raw_fd(), &self.request);
        }
    }
}

#[cfg(feature = "tokio")]
mod tokio_stream {
    use super::*;

    use std::future::poll_fn;
    use std::task::{ready, Context, Poll};
    use tokio::io::unix::AsyncFd;

    /// An asynchronous stream of input events.
    ///
    /// This can be used by calling [`stream.next_event().await?`](Self::next_event), or if you
    /// need to pass it as a stream somewhere, the [`futures::Stream`](Stream) implementation.
    /// There's also a lower-level [`Self::poll_event`] function if you need to fetch an event from
    /// inside a `Future::poll` impl.
    pub struct VirtualEventStream {
        device: AsyncFd<VirtualDevice>,
        index: usize,
    }
    impl Unpin for VirtualEventStream {}

    impl VirtualEventStream {
        pub(crate) fn new(device: VirtualDevice) -> io::Result<Self> {
            use nix::fcntl;
            fcntl::fcntl(device.as_raw_fd(), fcntl::F_SETFL(fcntl::OFlag::O_NONBLOCK))?;
            let device = AsyncFd::new(device)?;
            Ok(Self { device, index: 0 })
        }

        /// Returns a reference to the underlying device
        pub fn device(&self) -> &VirtualDevice {
            self.device.get_ref()
        }

        /// Returns a mutable reference to the underlying device.
        pub fn device_mut(&mut self) -> &mut VirtualDevice {
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
                    return Poll::Ready(Ok(InputEvent::from(ev)));
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

    #[cfg(feature = "stream-trait")]
    impl futures_core::Stream for VirtualEventStream {
        type Item = io::Result<InputEvent>;
        fn poll_next(
            self: std::pin::Pin<&mut Self>,
            cx: &mut Context<'_>,
        ) -> Poll<Option<Self::Item>> {
            self.get_mut().poll_event(cx).map(Some)
        }
    }
}
#[cfg(feature = "tokio")]
pub use tokio_stream::VirtualEventStream;
