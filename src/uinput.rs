//! Virtual device emulation for evdev via uinput.
//!
//! This is quite useful when testing/debugging devices, or synchronization.

use crate::compat::{input_event, input_id, uinput_abs_setup, uinput_setup, UINPUT_MAX_NAME_SIZE};
use crate::constants::{EventType, UInputEventType};
use crate::ff::FFEffectData;
use crate::inputid::{BusType, InputId};
use crate::raw_stream::vec_spare_capacity_mut;
use crate::{
    sys, AttributeSetRef, Error, FFEffectType, InputEvent, InputEventKind, Key, RelativeAxisType,
    SwitchType, UinputAbsSetup,
};
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::io::AsRawFd;
use std::os::unix::prelude::RawFd;
use std::path::PathBuf;
use std::time::SystemTime;

const UINPUT_PATH: &str = "/dev/uinput";
const SYSFS_PATH: &str = "/sys/devices/virtual/input";
const DEV_PATH: &str = "/dev/input";

#[derive(Debug)]
pub struct VirtualDeviceBuilder<'a> {
    file: File,
    name: &'a [u8],
    id: Option<input_id>,
    ff_effects_max: u32,
}

impl<'a> VirtualDeviceBuilder<'a> {
    pub fn new() -> io::Result<Self> {
        let mut options = OpenOptions::new();

        // Open in read-write mode.
        let file = options.read(true).write(true).open(UINPUT_PATH)?;

        Ok(VirtualDeviceBuilder {
            file,
            name: Default::default(),
            id: None,
            ff_effects_max: 0,
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
            sys::ui_abs_setup(self.file.as_raw_fd(), &axis.0 as *const uinput_abs_setup)?;
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

    pub fn with_ff(self, ff: &AttributeSetRef<FFEffectType>) -> io::Result<Self> {
        unsafe {
            sys::ui_set_evbit(
                self.file.as_raw_fd(),
                crate::EventType::FORCEFEEDBACK.0 as nix::sys::ioctl::ioctl_param_type,
            )?;
        }

        for bit in ff.iter() {
            unsafe {
                sys::ui_set_ffbit(
                    self.file.as_raw_fd(),
                    bit.0 as nix::sys::ioctl::ioctl_param_type,
                )?;
            }
        }

        Ok(self)
    }

    pub fn with_ff_effects_max(mut self, ff_effects_max: u32) -> Self {
        self.ff_effects_max = ff_effects_max;
        self
    }

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

        VirtualDevice::new(self.file, &usetup)
    }
}

const DEFAULT_ID: input_id = input_id {
    bustype: BusType::BUS_USB.0,
    vendor: 0x1234,  /* sample vendor */
    product: 0x5678, /* sample product */
    version: 0x111,
};

pub struct VirtualDevice {
    file: File,
    pub(crate) event_buf: Vec<input_event>,
}

impl VirtualDevice {
    /// Create a new virtual device.
    fn new(file: File, usetup: &uinput_setup) -> io::Result<Self> {
        unsafe { sys::ui_dev_setup(file.as_raw_fd(), usetup)? };
        unsafe { sys::ui_dev_create(file.as_raw_fd())? };

        Ok(VirtualDevice {
            file,
            event_buf: vec![],
        })
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

        Ok(DevNodesBlocking { dir })
    }

    /// Get the syspaths of the corresponding device nodes in /dev/input.
    #[cfg(feature = "tokio")]
    pub async fn enumerate_dev_nodes(&mut self) -> io::Result<DevNodes> {
        let path = self.get_syspath()?;
        let dir = tokio_1::fs::read_dir(path).await?;

        Ok(DevNodes { dir })
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

    /// Processes the given [`UInputEvent`] if it is a force feedback upload event, in which case
    /// this function will start the force feedback upload and claim ownership over the
    /// [`UInputEvent`] and return a [`FFUploadEvent`] instead.
    ///
    /// The returned event allows the user to allocate and set the effect ID as well as access the
    /// effect data.
    pub fn process_ff_upload(&mut self, event: UInputEvent) -> Result<FFUploadEvent, Error> {
        if event.kind() != InputEventKind::UInput(UInputEventType::UI_FF_UPLOAD.0) {
            return Err(Error::InvalidEvent);
        }

        let mut request: sys::uinput_ff_upload = unsafe { std::mem::zeroed() };
        request.request_id = event.value() as u32;
        unsafe { sys::ui_begin_ff_upload(self.file.as_raw_fd(), &mut request)? };

        request.retval = 0;

        let file = self.file.try_clone()?;

        Ok(FFUploadEvent { file, request })
    }

    /// Processes the given [`UInputEvent`] if it is a force feedback erase event, in which case
    /// this function will start the force feedback erasure and claim ownership over the
    /// [`UInputEvent`] and return a [`FFEraseEvent`] instead.
    ///
    /// The returned event allows the user to access the effect ID, such that it can free any
    /// memory used for the given effect ID.
    pub fn process_ff_erase(&mut self, event: UInputEvent) -> Result<FFEraseEvent, Error> {
        if event.kind() != InputEventKind::UInput(UInputEventType::UI_FF_ERASE.0) {
            return Err(Error::InvalidEvent);
        }

        let mut request: sys::uinput_ff_erase = unsafe { std::mem::zeroed() };
        request.request_id = event.value() as u32;
        unsafe { sys::ui_begin_ff_erase(self.file.as_raw_fd(), &mut request)? };

        request.retval = 0;

        let file = self.file.try_clone()?;

        Ok(FFEraseEvent { file, request })
    }

    /// Read a maximum of `num` events into the internal buffer. If the underlying fd is not
    /// O_NONBLOCK, this will block.
    ///
    /// Returns the number of events that were read, or an error.
    pub(crate) fn fill_events(&mut self) -> io::Result<usize> {
        let fd = self.file.as_raw_fd();
        self.event_buf.reserve(crate::EVENT_BATCH_SIZE);

        // TODO: use Vec::spare_capacity_mut or Vec::split_at_spare_mut when they stabilize
        let spare_capacity = vec_spare_capacity_mut(&mut self.event_buf);
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
    pub fn fetch_events(&mut self) -> io::Result<impl Iterator<Item = UInputEvent> + '_> {
        self.fill_events()?;
        Ok(self.event_buf.drain(..).map(InputEvent).map(UInputEvent))
    }

    #[cfg(feature = "tokio")]
    #[inline]
    pub fn into_event_stream(self) -> io::Result<VirtualEventStream> {
        VirtualEventStream::new(self)
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
        for entry in self.dir.by_ref() {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => return Some(Err(e)),
            };

            // Map the directory name to its file name.
            let name = entry.file_name().to_string_lossy().to_owned().to_string();

            // Ignore file names that do not start with event.
            if !name.starts_with("event") {
                continue;
            }

            // Construct the path of the form '/dev/input/eventX'.
            let mut path: PathBuf = PathBuf::from(DEV_PATH);
            path.push(name);

            return Some(Ok(path));
        }

        None
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
            let path = self
                .dir
                .next_entry()
                .await?
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

impl AsRawFd for VirtualDevice {
    fn as_raw_fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }
}

/// An event from a virtual uinput device.
#[derive(Debug)]
pub struct UInputEvent(InputEvent);

impl UInputEvent {
    /// Returns the timestamp associated with the event.
    #[inline]
    pub fn timestamp(&self) -> SystemTime {
        self.0.timestamp()
    }

    /// Returns the type of event this describes, e.g. Key, Switch, etc.
    #[inline]
    pub fn event_type(&self) -> EventType {
        self.0.event_type()
    }

    /// Returns the raw "code" field directly from input_event.
    #[inline]
    pub fn code(&self) -> u16 {
        self.0.code()
    }

    /// A convenience function to return `self.code()` wrapped in a certain newtype determined by
    /// the type of this event.
    ///
    /// This is useful if you want to match events by specific key codes or axes. Note that this
    /// does not capture the event value, just the type and code.
    #[inline]
    pub fn kind(&self) -> InputEventKind {
        self.0.kind()
    }

    /// Returns the raw "value" field directly from input_event.
    ///
    /// For keys and switches the values 0 and 1 map to pressed and not pressed respectively.
    /// For axes, the values depend on the hardware and driver implementation.
    #[inline]
    pub fn value(&self) -> i32 {
        self.0.value()
    }
}

/// Represents a force feedback upload event that we are currently processing.
pub struct FFUploadEvent {
    file: File,
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
            let _ = sys::ui_end_ff_upload(self.file.as_raw_fd(), &self.request);
        }
    }
}

/// Represents a force feedback erase event that we are currently processing.
pub struct FFEraseEvent {
    file: File,
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
            let _ = sys::ui_end_ff_erase(self.file.as_raw_fd(), &self.request);
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
        pub async fn next_event(&mut self) -> io::Result<UInputEvent> {
            poll_fn(|cx| self.poll_event(cx)).await
        }

        /// A lower-level function for directly polling this stream.
        pub fn poll_event(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<UInputEvent>> {
            'outer: loop {
                if let Some(&ev) = self.device.get_ref().event_buf.get(self.index) {
                    self.index += 1;
                    return Poll::Ready(Ok(UInputEvent(InputEvent(ev))));
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

    impl Stream for VirtualEventStream {
        type Item = io::Result<UInputEvent>;
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
pub use tokio_stream::VirtualEventStream;
