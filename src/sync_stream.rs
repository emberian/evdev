use crate::compat::{input_absinfo, input_event};
use crate::constants::*;
use crate::device_state::DeviceState;
use crate::ff::*;
use crate::raw_stream::{FFEffect, RawDevice};
use crate::{AttributeSet, AttributeSetRef, AutoRepeat, InputEvent, InputEventKind, InputId, Key};
use std::os::unix::io::{AsRawFd, RawFd};
use std::path::Path;
use std::time::SystemTime;
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
/// [`Device::cached_state()`] will be kept up to date when `fetch_events()` is called.
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

    #[inline]
    fn _open(path: &Path) -> io::Result<Device> {
        RawDevice::open(path).map(Self::from_raw_device)
    }

    // TODO: should this be public?
    pub(crate) fn from_raw_device(raw: RawDevice) -> Device {
        let state = DeviceState::new(&raw);
        let prev_state = state.clone();

        Device {
            raw,
            prev_state,
            state,
            block_dropped: false,
        }
    }

    /// Returns the synchronization engine's current understanding (cache) of the device state.
    ///
    /// Note that this represents the internal cache of the synchronization engine as of the last
    /// entry that was pulled out. The advantage to calling this instead of invoking
    /// [`get_key_state`](RawDevice::get_key_state)
    /// and the like directly is speed: because reading this cache doesn't require any syscalls it's
    /// easy to do inside a tight loop. The downside is that if the stream is not being driven quickly,
    /// this can very quickly get desynchronized from the kernel and provide inaccurate data.
    pub fn cached_state(&self) -> &DeviceState {
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

    /// Returns a struct containing the delay and period for auto repeat
    pub fn get_auto_repeat(&self) -> Option<AutoRepeat> {
        self.raw.get_auto_repeat()
    }

    /// Update the delay and period for autorepeat
    pub fn update_auto_repeat(&mut self, repeat: &AutoRepeat) -> io::Result<()> {
        self.raw.update_auto_repeat(repeat)
    }

    /// Retrieve the scancode for a keycode, if any
    pub fn get_scancode_by_keycode(&self, keycode: Key) -> io::Result<Vec<u8>> {
        self.raw.get_scancode_by_keycode(keycode.code() as u32)
    }

    /// Retrieve the keycode and scancode by index, starting at 0
    pub fn get_scancode_by_index(&self, index: u16) -> io::Result<(u32, Vec<u8>)> {
        self.raw.get_scancode_by_index(index)
    }

    /// Update a scancode. The return value is the previous keycode
    pub fn update_scancode(&self, keycode: Key, scancode: &[u8]) -> io::Result<Key> {
        self.raw
            .update_scancode(keycode.code() as u32, scancode)
            .map(|keycode| Key::new(keycode as u16))
    }

    /// Update a scancode by index. The return value is the previous keycode
    pub fn update_scancode_by_index(
        &self,
        index: u16,
        keycode: Key,
        scancode: &[u8],
    ) -> io::Result<u32> {
        self.raw
            .update_scancode_by_index(index, keycode.code() as u32, scancode)
    }

    /// Returns the set of supported "properties" for the device (see `INPUT_PROP_*` in kernel headers)
    pub fn properties(&self) -> &AttributeSetRef<PropType> {
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
    pub fn supported_events(&self) -> &AttributeSetRef<EventType> {
        self.raw.supported_events()
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
        self.raw.supported_keys()
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
        self.raw.supported_relative_axes()
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
        self.raw.supported_switches()
    }

    /// Returns a set of supported LEDs on the device.
    ///
    /// Most commonly these are state indicator lights for things like Scroll Lock, but they
    /// can also be found in cameras and other devices.
    pub fn supported_leds(&self) -> Option<&AttributeSetRef<LedType>> {
        self.raw.supported_leds()
    }

    /// Returns a set of supported "miscellaneous" capabilities.
    ///
    /// Aside from vendor-specific key scancodes, most of these are uncommon.
    pub fn misc_properties(&self) -> Option<&AttributeSetRef<MiscType>> {
        self.raw.misc_properties()
    }

    /// Returns the set of supported force feedback effects supported by a device.
    pub fn supported_ff(&self) -> Option<&AttributeSetRef<FFEffectType>> {
        self.raw.supported_ff()
    }

    /// Returns the set of supported simple sounds supported by a device.
    ///
    /// You can use these to make really annoying beep sounds come from an internal self-test
    /// speaker, for instance.
    pub fn supported_sounds(&self) -> Option<&AttributeSetRef<SoundType>> {
        self.raw.supported_sounds()
    }

    /// Retrieve the current keypress state directly via kernel syscall.
    pub fn get_key_state(&self) -> io::Result<AttributeSet<Key>> {
        self.raw.get_key_state()
    }

    /// Retrieve the current absolute axis state directly via kernel syscall.
    pub fn get_abs_state(&self) -> io::Result<[input_absinfo; AbsoluteAxisType::COUNT]> {
        self.raw.get_abs_state()
    }

    /// Retrieve the current switch state directly via kernel syscall.
    pub fn get_switch_state(&self) -> io::Result<AttributeSet<SwitchType>> {
        self.raw.get_switch_state()
    }

    /// Retrieve the current LED state directly via kernel syscall.
    pub fn get_led_state(&self) -> io::Result<AttributeSet<LedType>> {
        self.raw.get_led_state()
    }

    fn sync_state(&mut self, now: SystemTime) -> io::Result<()> {
        if let Some(ref mut key_vals) = self.state.key_vals {
            self.raw.update_key_state(key_vals)?;
        }
        if let Some(ref mut abs_vals) = self.state.abs_vals {
            self.raw.update_abs_state(abs_vals)?;
        }
        if let Some(ref mut switch_vals) = self.state.switch_vals {
            self.raw.update_switch_state(switch_vals)?;
        }
        if let Some(ref mut led_vals) = self.state.led_vals {
            self.raw.update_led_state(led_vals)?;
        }
        self.state.timestamp = now;
        Ok(())
    }

    fn fetch_events_inner(&mut self) -> io::Result<Option<SyncState>> {
        let block_dropped = std::mem::take(&mut self.block_dropped);
        let sync = if block_dropped {
            self.prev_state.clone_from(&self.state);
            let now = SystemTime::now();
            self.sync_state(now)?;
            Some(SyncState::Keys {
                time: crate::systime_to_timeval(&now),
                start: Key::new(0),
            })
        } else {
            None
        };

        self.raw.fill_events()?;

        Ok(sync)
    }

    /// Fetches and returns events from the kernel ring buffer, doing synchronization on SYN_DROPPED.
    ///
    /// By default this will block until events are available. Typically, users will want to call
    /// this in a tight loop within a thread.
    /// Will insert "fake" events.
    pub fn fetch_events(&mut self) -> io::Result<FetchEventsSynced<'_>> {
        let sync = self.fetch_events_inner()?;

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

    /// Grab the device through a kernel syscall.
    ///
    /// This prevents other clients (including kernel-internal ones such as rfkill) from receiving
    /// events from this device.
    pub fn grab(&mut self) -> io::Result<()> {
        self.raw.grab()
    }

    /// Ungrab the device through a kernel syscall.
    pub fn ungrab(&mut self) -> io::Result<()> {
        self.raw.ungrab()
    }

    /// Send an event to the device.
    ///
    /// Events that are typically sent to devices are
    /// [EventType::LED] (turn device LEDs on and off),
    /// [EventType::SOUND] (play a sound on the device)
    /// and [EventType::FORCEFEEDBACK] (play force feedback effects on the device, i.e. rumble).
    pub fn send_events(&mut self, events: &[InputEvent]) -> io::Result<()> {
        self.raw.send_events(events)
    }

    /// Uploads a force feedback effect to the device.
    pub fn upload_ff_effect(&mut self, data: FFEffectData) -> io::Result<FFEffect> {
        self.raw.upload_ff_effect(data)
    }

    /// Sets the force feedback gain, i.e. how strong the force feedback effects should be for the
    /// device. A gain of 0 means no gain, whereas `u16::MAX` is the maximum gain.
    pub fn set_ff_gain(&mut self, value: u16) -> io::Result<()> {
        self.raw.set_ff_gain(value)
    }

    /// Enables or disables autocenter for the force feedback device.
    pub fn set_ff_autocenter(&mut self, value: u16) -> io::Result<()> {
        self.raw.set_ff_autocenter(value)
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
                        let ev = InputEvent(input_event {
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
                    &AttributeSetRef<Key>,
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
                    &[input_absinfo],
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
                    &AttributeSetRef<SwitchType>,
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
                    &AttributeSetRef<LedType>,
                    |st| st.led_vals().unwrap(),
                    |vals, led| vals.contains(led)
                );
                let ev = InputEvent(input_event {
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
        if let Some(ev) = compensate_events(&mut self.sync, self.dev) {
            self.dev.prev_state.process_event(ev);
            return Some(ev);
        }
        let state = &mut self.dev.state;
        let (res, consumed_to) = sync_events(&mut self.range, &self.dev.raw.event_buf, |ev| {
            state.process_event(ev)
        });
        if let Some(end) = consumed_to {
            self.consumed_to = end
        }
        match res {
            Ok(ev) => Some(InputEvent(ev)),
            Err(requires_sync) => {
                if requires_sync {
                    self.dev.block_dropped = true;
                }
                None
            }
        }
    }
}

impl<'a> Drop for FetchEventsSynced<'a> {
    fn drop(&mut self) {
        self.dev.raw.event_buf.drain(..self.consumed_to);
    }
}

/// Err(true) means the device should sync the state with ioctl
#[inline]
fn sync_events(
    range: &mut std::ops::Range<usize>,
    event_buf: &[input_event],
    mut handle_event: impl FnMut(InputEvent),
) -> (Result<input_event, bool>, Option<usize>) {
    let mut consumed_to = None;
    let res = 'outer: loop {
        if let Some(idx) = range.next() {
            // we're going through and emitting the events of a block that we checked
            break Ok(event_buf[idx]);
        }
        // find the range of this new block: look for a SYN_REPORT
        let block_start = range.end;
        let mut block_dropped = false;
        for (i, ev) in event_buf.iter().enumerate().skip(block_start) {
            let ev = InputEvent(*ev);
            match ev.kind() {
                InputEventKind::Synchronization(Synchronization::SYN_DROPPED) => {
                    block_dropped = true;
                }
                InputEventKind::Synchronization(Synchronization::SYN_REPORT) => {
                    consumed_to = Some(i + 1);
                    if block_dropped {
                        *range = event_buf.len()..event_buf.len();
                        break 'outer Err(true);
                    } else {
                        *range = block_start..i + 1;
                        continue 'outer;
                    }
                }
                _ => handle_event(ev),
            }
        }
        break Err(false);
    };
    (res, consumed_to)
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

    use crate::raw_stream::poll_fn;
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
        device: AsyncFd<Device>,
        event_range: std::ops::Range<usize>,
        consumed_to: usize,
        sync: Option<SyncState>,
    }
    impl Unpin for EventStream {}

    impl EventStream {
        pub(crate) fn new(device: Device) -> io::Result<Self> {
            use nix::fcntl;
            fcntl::fcntl(device.as_raw_fd(), fcntl::F_SETFL(fcntl::OFlag::O_NONBLOCK))?;
            let device = AsyncFd::new(device)?;
            Ok(Self {
                device,
                event_range: 0..0,
                consumed_to: 0,
                sync: None,
            })
        }

        /// Returns a reference to the underlying device
        pub fn device(&self) -> &Device {
            self.device.get_ref()
        }

        /// Returns a mutable reference to the underlying device
        pub fn device_mut(&mut self) -> &mut Device {
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
                let dev = self.device.get_mut();
                if let Some(ev) = compensate_events(&mut self.sync, dev) {
                    return Poll::Ready(Ok(ev));
                }
                let state = &mut dev.state;
                let (res, consumed_to) =
                    sync_events(&mut self.event_range, &dev.raw.event_buf, |ev| {
                        state.process_event(ev)
                    });
                if let Some(end) = consumed_to {
                    self.consumed_to = end
                }
                match res {
                    Ok(ev) => return Poll::Ready(Ok(InputEvent(ev))),
                    Err(requires_sync) => {
                        if requires_sync {
                            dev.block_dropped = true;
                        }
                    }
                }
                dev.raw.event_buf.drain(..self.consumed_to);
                self.consumed_to = 0;

                loop {
                    let mut guard = ready!(self.device.poll_read_ready_mut(cx))?;

                    let res = guard.try_io(|device| device.get_mut().fetch_events_inner());
                    match res {
                        Ok(res) => {
                            self.sync = res?;
                            self.event_range = 0..0;
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
}
#[cfg(feature = "tokio")]
pub use tokio_stream::EventStream;

#[cfg(test)]
mod tests {
    use super::*;

    fn result_events_iter(
        events: &[input_event],
    ) -> impl Iterator<Item = Result<input_event, ()>> + '_ {
        let mut range = 0..0;
        std::iter::from_fn(move || {
            let (res, _) = sync_events(&mut range, events, |_| {});
            match res {
                Ok(x) => Some(Ok(x)),
                Err(true) => Some(Err(())),
                Err(false) => None,
            }
        })
    }

    fn events_iter(events: &[input_event]) -> impl Iterator<Item = input_event> + '_ {
        result_events_iter(events).flatten()
    }

    #[allow(non_upper_case_globals)]
    const time: libc::timeval = libc::timeval {
        tv_sec: 0,
        tv_usec: 0,
    };
    const KEY4: input_event = input_event {
        time,
        type_: EventType::KEY.0,
        code: Key::KEY_4.0,
        value: 1,
    };
    const REPORT: input_event = input_event {
        time,
        type_: EventType::SYNCHRONIZATION.0,
        code: Synchronization::SYN_REPORT.0,
        value: 0,
    };
    const DROPPED: input_event = input_event {
        code: Synchronization::SYN_DROPPED.0,
        ..REPORT
    };

    #[test]
    fn test_sync_impl() {
        itertools::assert_equal(events_iter(&[]), vec![]);
        itertools::assert_equal(events_iter(&[KEY4]), vec![]);
        itertools::assert_equal(events_iter(&[KEY4, REPORT]), vec![KEY4, REPORT]);
        itertools::assert_equal(events_iter(&[KEY4, REPORT, KEY4]), vec![KEY4, REPORT]);
        itertools::assert_equal(
            result_events_iter(&[KEY4, REPORT, KEY4, DROPPED, REPORT]),
            vec![Ok(KEY4), Ok(REPORT), Err(())],
        );
    }

    #[test]
    fn test_iter_consistency() {
        // once it sees a SYN_DROPPED, it shouldn't mark the block after it as consumed even if we
        // keep calling the iterator like an idiot
        let evs = &[KEY4, REPORT, DROPPED, REPORT, KEY4, REPORT, KEY4];
        let mut range = 0..0;
        let mut next = || sync_events(&mut range, evs, |_| {});
        assert_eq!(next(), (Ok(KEY4), Some(2)));
        assert_eq!(next(), (Ok(REPORT), None));
        assert_eq!(next(), (Err(true), Some(4)));
        assert_eq!(next(), (Err(false), None));
        assert_eq!(next(), (Err(false), None));
        assert_eq!(next(), (Err(false), None));
    }
}
