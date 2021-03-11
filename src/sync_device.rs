use crate::constants::*;
use crate::raw_events::RawDevice;
use crate::{AttributeSet, DeviceState, InputEvent, InputEventKind, InputId, Key};
use std::os::unix::io::{AsRawFd, RawFd};
use std::path::Path;
use std::{fmt, io};

/// A physical or virtual device supported by evdev.
///
/// Each device corresponds to a path typically found in `/dev/input`, and supports access via
/// one or more "types". For example, an optical mouse has buttons that are represented by "keys",
/// and reflects changes in its position via "relative axis" reports.
///
/// This type specifically is a wrapper over [`RawDevice`],that synchronizes with the kernel's
/// state when events are dropped.
///
/// If `fetch_events()` isn't called often enough and the kernel drops events from its internal
/// buffer, synthetic events will be injected into the iterator returned by `fetch_events()` and
/// [`Device::state()`] will be kept up to date when `fetch_events()` is called.
pub struct Device {
    raw: RawDevice,
    prev_state: DeviceState,
    state: DeviceState,
    block_dropped: bool,
}

impl Device {
    /// Opens a device, given its system path.
    ///
    /// Paths are typically something like `/dev/input/event0`.
    #[inline(always)]
    pub fn open(path: impl AsRef<Path>) -> io::Result<Device> {
        Self::_open(path.as_ref())
    }

    fn _open(path: &Path) -> io::Result<Device> {
        let raw = RawDevice::open(path)?;

        let state = raw.empty_state();
        let prev_state = state.clone();

        Ok(Device {
            raw,
            prev_state,
            state,
            block_dropped: false,
        })
    }

    pub fn state(&self) -> &DeviceState {
        &self.state
    }

    /// Returns the device's name as read from the kernel.
    pub fn name(&self) -> Option<&str> {
        self.raw.name()
    }

    /// Returns the device's physical location, either as set by the caller or as read from the kernel.
    pub fn physical_path(&self) -> Option<&str> {
        self.raw.physical_path()
    }

    /// Returns the user-defined "unique name" of the device, if one has been set.
    pub fn unique_name(&self) -> Option<&str> {
        self.raw.unique_name()
    }

    /// Returns a struct containing bustype, vendor, product, and version identifiers
    pub fn input_id(&self) -> InputId {
        self.raw.input_id()
    }

    /// Returns the set of supported "properties" for the device (see `INPUT_PROP_*` in kernel headers)
    pub fn properties(&self) -> AttributeSet<'_, PropType> {
        self.raw.properties()
    }

    /// Returns a tuple of the driver version containing major, minor, rev
    pub fn driver_version(&self) -> (u8, u8, u8) {
        self.raw.driver_version()
    }

    /// Returns a set of the event types supported by this device (Key, Switch, etc)
    ///
    /// If you're interested in the individual keys or switches supported, it's probably easier
    /// to just call the appropriate `supported_*` function instead.
    pub fn supported_events(&self) -> AttributeSet<'_, EventType> {
        self.raw.supported_events()
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
    pub fn supported_keys(&self) -> Option<AttributeSet<'_, Key>> {
        self.raw.supported_keys()
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
    pub fn supported_relative_axes(&self) -> Option<AttributeSet<'_, RelativeAxisType>> {
        self.raw.supported_relative_axes()
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
    pub fn supported_absolute_axes(&self) -> Option<AttributeSet<'_, AbsoluteAxisType>> {
        self.raw.supported_absolute_axes()
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
    pub fn supported_switches(&self) -> Option<AttributeSet<'_, SwitchType>> {
        self.raw.supported_switches()
    }

    /// Returns a set of supported LEDs on the device.
    ///
    /// Most commonly these are state indicator lights for things like Scroll Lock, but they
    /// can also be found in cameras and other devices.
    pub fn supported_leds(&self) -> Option<AttributeSet<'_, LedType>> {
        self.raw.supported_leds()
    }

    /// Returns a set of supported "miscellaneous" capabilities.
    ///
    /// Aside from vendor-specific key scancodes, most of these are uncommon.
    pub fn misc_properties(&self) -> Option<AttributeSet<'_, MiscType>> {
        self.raw.misc_properties()
    }

    // pub fn supported_repeats(&self) -> Option<Repeat> {
    //     self.rep
    // }

    /// Returns the set of supported simple sounds supported by a device.
    ///
    /// You can use these to make really annoying beep sounds come from an internal self-test
    /// speaker, for instance.
    pub fn supported_sounds(&self) -> Option<AttributeSet<'_, SoundType>> {
        self.raw.supported_sounds()
    }

    /// Fetches and returns events from the kernel ring buffer, doing synchronization on SYN_DROPPED.
    ///
    /// By default this will block until events are available. Typically, users will want to call
    /// this in a tight loop within a thread.
    /// Will insert "fake" events.
    pub fn fetch_events(&mut self) -> io::Result<FetchEventsSynced<'_>> {
        let block_dropped = std::mem::take(&mut self.block_dropped);
        let sync = if block_dropped {
            self.prev_state.clone_from(&self.state);
            self.raw.sync_state(&mut self.state)?;
            Some(SyncState::Keys {
                time: crate::systime_to_timeval(&std::time::SystemTime::now()),
                start: Key::new(0),
            })
        } else {
            None
        };

        self.raw.fill_events()?;

        Ok(FetchEventsSynced {
            dev: self,
            range: 0..0,
            consumed_to: 0,
            sync,
        })
    }

    #[cfg(feature = "tokio")]
    pub fn into_event_stream(self) -> io::Result<EventStream> {
        EventStream::new(self)
    }
}

impl AsRawFd for Device {
    fn as_raw_fd(&self) -> RawFd {
        self.raw.as_raw_fd()
    }
}

/// An iterator over events of a [`Device`], produced by [`Device::fetch_events`].
pub struct FetchEventsSynced<'a> {
    dev: &'a mut Device,
    /// The current block of the events we're returning to the consumer. If empty
    /// (i.e. for any x, range == x..x) then we'll find another block on the next `next()` call.
    range: std::ops::Range<usize>,
    /// The index into dev.raw.event_buf up to which we'll delete events when dropped.
    consumed_to: usize,
    /// Our current synchronization state, i.e. whether we're currently diffing key_vals,
    /// abs_vals, switch_vals, led_vals, or none of them.
    sync: Option<SyncState>,
}

enum SyncState {
    Keys {
        time: libc::timeval,
        start: Key,
    },
    Absolutes {
        time: libc::timeval,
        start: AbsoluteAxisType,
    },
    Switches {
        time: libc::timeval,
        start: SwitchType,
    },
    Leds {
        time: libc::timeval,
        start: LedType,
    },
}

#[inline]
fn compensate_events(state: &mut Option<SyncState>, dev: &mut Device) -> Option<InputEvent> {
    let sync = state.as_mut()?;
    // this macro checks if there are any differences between the old state and the new for the
    // specific substate(?) that we're checking and if so returns an input_event with the value set
    // to the value from the up-to-date state
    macro_rules! try_compensate {
        ($time:expr, $start:ident : $typ:ident, $evtype:ident, $sync:ident, $supporteds:ident, $state:ty, $get_state:expr, $get_value:expr) => {
            if let Some(supported_types) = dev.$supporteds() {
                let types_to_check = supported_types.slice(*$start);
                let get_state: fn(&DeviceState) -> $state = $get_state;
                let vals = get_state(&dev.state);
                let old_vals = get_state(&dev.prev_state);
                let get_value: fn($state, $typ) -> _ = $get_value;
                for typ in types_to_check.iter() {
                    let prev = get_value(old_vals, typ);
                    let value = get_value(vals, typ);
                    if prev != value {
                        $start.0 = typ.0 + 1;
                        let ev = InputEvent(libc::input_event {
                            time: *$time,
                            type_: EventType::$evtype.0,
                            code: typ.0,
                            value: value as _,
                        });
                        return Some(ev);
                    }
                }
            }
        };
    }
    loop {
        // check keys, then abs axes, then switches, then leds
        match sync {
            SyncState::Keys { time, start } => {
                try_compensate!(
                    time,
                    start: Key,
                    KEY,
                    Keys,
                    supported_keys,
                    AttributeSet<Key>,
                    |st| st.key_vals().unwrap(),
                    |vals, key| vals.contains(key)
                );
                *sync = SyncState::Absolutes {
                    time: *time,
                    start: AbsoluteAxisType(0),
                };
                continue;
            }
            SyncState::Absolutes { time, start } => {
                try_compensate!(
                    time,
                    start: AbsoluteAxisType,
                    ABSOLUTE,
                    Absolutes,
                    supported_absolute_axes,
                    &[libc::input_absinfo],
                    |st| st.abs_vals().unwrap(),
                    |vals, abs| vals[abs.0 as usize].value
                );
                *sync = SyncState::Switches {
                    time: *time,
                    start: SwitchType(0),
                };
                continue;
            }
            SyncState::Switches { time, start } => {
                try_compensate!(
                    time,
                    start: SwitchType,
                    SWITCH,
                    Switches,
                    supported_switches,
                    AttributeSet<SwitchType>,
                    |st| st.switch_vals().unwrap(),
                    |vals, sw| vals.contains(sw)
                );
                *sync = SyncState::Leds {
                    time: *time,
                    start: LedType(0),
                };
                continue;
            }
            SyncState::Leds { time, start } => {
                try_compensate!(
                    time,
                    start: LedType,
                    LED,
                    Leds,
                    supported_leds,
                    AttributeSet<LedType>,
                    |st| st.led_vals().unwrap(),
                    |vals, led| vals.contains(led)
                );
                let ev = InputEvent(libc::input_event {
                    time: *time,
                    type_: EventType::SYNCHRONIZATION.0,
                    code: Synchronization::SYN_REPORT.0,
                    value: 0,
                });
                *state = None;
                return Some(ev);
            }
        }
    }
}

impl<'a> Iterator for FetchEventsSynced<'a> {
    type Item = InputEvent;
    fn next(&mut self) -> Option<InputEvent> {
        // first: check if we need to emit compensatory events due to a SYN_DROPPED we found in the
        // last batch of blocks
        if let Some(ev) = compensate_events(&mut self.sync, &mut self.dev) {
            return Some(ev);
        }
        'outer: loop {
            if let Some(idx) = self.range.next() {
                // we're going through and emitting the events of a block that we checked
                return Some(InputEvent(self.dev.raw.event_buf[idx]));
            }
            // find the range of this new block: look for a SYN_REPORT
            let block_start = self.range.end + 1;
            let mut block_dropped = false;
            for (i, ev) in self.dev.raw.event_buf.iter().enumerate().skip(block_start) {
                let ev = InputEvent(*ev);
                match ev.kind() {
                    InputEventKind::Synchronization(Synchronization::SYN_DROPPED) => {
                        block_dropped = true;
                    }
                    InputEventKind::Synchronization(Synchronization::SYN_REPORT) => {
                        self.consumed_to = i + 1;
                        if block_dropped {
                            self.dev.block_dropped = true;
                            return None;
                        } else {
                            self.range = block_start..i + 1;
                            continue 'outer;
                        }
                    }
                    _ => self.dev.state.process_event(ev),
                }
            }
            return None;
        }
    }
}

impl<'a> Drop for FetchEventsSynced<'a> {
    fn drop(&mut self) {
        self.dev.raw.event_buf.drain(..self.consumed_to);
    }
}

impl fmt::Display for Device {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "{}:", self.name().unwrap_or("Unnamed device"))?;
        let (maj, min, pat) = self.driver_version();
        writeln!(f, "  Driver version: {}.{}.{}", maj, min, pat)?;
        if let Some(ref phys) = self.physical_path() {
            writeln!(f, "  Physical address: {:?}", phys)?;
        }
        if let Some(ref uniq) = self.unique_name() {
            writeln!(f, "  Unique name: {:?}", uniq)?;
        }

        let id = self.input_id();

        writeln!(f, "  Bus: {}", id.bus_type())?;
        writeln!(f, "  Vendor: {:#x}", id.vendor())?;
        writeln!(f, "  Product: {:#x}", id.product())?;
        writeln!(f, "  Version: {:#x}", id.version())?;
        writeln!(f, "  Properties: {:?}", self.properties())?;

        if let (Some(supported_keys), Some(key_vals)) =
            (self.supported_keys(), self.state.key_vals())
        {
            writeln!(f, "  Keys supported:")?;
            for key in supported_keys.iter() {
                let key_idx = key.code() as usize;
                writeln!(
                    f,
                    "    {:?} ({}index {})",
                    key,
                    if key_vals.contains(key) {
                        "pressed, "
                    } else {
                        ""
                    },
                    key_idx
                )?;
            }
        }

        if let Some(supported_relative) = self.supported_relative_axes() {
            writeln!(f, "  Relative Axes: {:?}", supported_relative)?;
        }

        if let (Some(supported_abs), Some(abs_vals)) =
            (self.supported_absolute_axes(), &self.state.abs_vals)
        {
            writeln!(f, "  Absolute Axes:")?;
            for abs in supported_abs.iter() {
                writeln!(
                    f,
                    "    {:?} ({:?}, index {})",
                    abs, abs_vals[abs.0 as usize], abs.0
                )?;
            }
        }

        if let Some(supported_misc) = self.misc_properties() {
            writeln!(f, "  Miscellaneous capabilities: {:?}", supported_misc)?;
        }

        if let (Some(supported_switch), Some(switch_vals)) =
            (self.supported_switches(), self.state.switch_vals())
        {
            writeln!(f, "  Switches:")?;
            for sw in supported_switch.iter() {
                writeln!(
                    f,
                    "    {:?} ({:?}, index {})",
                    sw,
                    switch_vals.contains(sw),
                    sw.0
                )?;
            }
        }

        if let (Some(supported_led), Some(led_vals)) =
            (self.supported_leds(), self.state.led_vals())
        {
            writeln!(f, "  LEDs:")?;
            for led in supported_led.iter() {
                writeln!(
                    f,
                    "    {:?} ({:?}, index {})",
                    led,
                    led_vals.contains(led),
                    led.0
                )?;
            }
        }

        if let Some(supported_snd) = self.supported_sounds() {
            write!(f, "  Sounds:")?;
            for snd in supported_snd.iter() {
                writeln!(f, "    {:?} (index {})", snd, snd.0)?;
            }
        }

        // if let Some(rep) = self.rep {
        //     writeln!(f, "  Repeats: {:?}", rep)?;
        // }

        let evs = self.supported_events();

        if evs.contains(EventType::FORCEFEEDBACK) {
            writeln!(f, "  Force Feedback supported")?;
        }

        if evs.contains(EventType::POWER) {
            writeln!(f, "  Power supported")?;
        }

        if evs.contains(EventType::FORCEFEEDBACKSTATUS) {
            writeln!(f, "  Force Feedback status supported")?;
        }

        Ok(())
    }
}

#[cfg(feature = "tokio")]
mod tokio_stream {
    use super::*;

    use tokio_1 as tokio;

    use crate::nix_err;
    use futures_core::{ready, Stream};
    use std::collections::VecDeque;
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
        device: AsyncFd<Device>,
        events: VecDeque<InputEvent>,
    }
    impl Unpin for EventStream {}

    impl EventStream {
        pub(crate) fn new(device: Device) -> io::Result<Self> {
            use nix::fcntl;
            fcntl::fcntl(device.as_raw_fd(), fcntl::F_SETFL(fcntl::OFlag::O_NONBLOCK))
                .map_err(nix_err)?;
            let device = AsyncFd::new(device)?;
            Ok(Self {
                device,
                events: VecDeque::new(),
            })
        }

        /// Returns a reference to the underlying device
        pub fn device(&self) -> &Device {
            self.device.get_ref()
        }

        /// Try to wait for the next event in this stream. Any errors are likely to be fatal, i.e.
        /// any calls afterwards will likely error as well.
        pub async fn next_event(&mut self) -> io::Result<InputEvent> {
            poll_fn(|cx| self.poll_event(cx)).await
        }

        /// A lower-level function for directly polling this stream.
        pub fn poll_event(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<InputEvent>> {
            let Self { device, events } = self;

            if let Some(ev) = events.pop_front() {
                return Poll::Ready(Ok(ev));
            }

            loop {
                let mut guard = ready!(device.poll_read_ready_mut(cx))?;

                let res = guard.try_io(|device| {
                    events.extend(device.get_mut().fetch_events()?);
                    Ok(())
                });
                match res {
                    Ok(res) => {
                        let () = res?;
                        let ret = match events.pop_front() {
                            Some(ev) => Poll::Ready(Ok(ev)),
                            None => Poll::Pending,
                        };
                        return ret;
                    }
                    Err(_would_block) => continue,
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
    fn poll_fn<T, F: FnMut(&mut Context<'_>) -> Poll<T> + Unpin>(f: F) -> PollFn<F> {
        PollFn(f)
    }
    struct PollFn<F>(F);
    impl<T, F: FnMut(&mut Context<'_>) -> Poll<T> + Unpin> std::future::Future for PollFn<F> {
        type Output = T;
        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
            (self.get_mut().0)(cx)
        }
    }
}
#[cfg(feature = "tokio")]
pub use tokio_stream::EventStream;
