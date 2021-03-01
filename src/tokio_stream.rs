use tokio_1 as tokio;

use crate::{nix_err, Device, InputEvent, DEFAULT_EVENT_COUNT};
use futures_core::{ready, Stream};
use std::io;
use std::os::unix::io::AsRawFd;
use std::pin::Pin;
use std::task::{Context, Poll};
use tokio::io::unix::AsyncFd;

/// An async stream of events.
pub struct EventStream {
    device: AsyncFd<Device>,
}
impl Unpin for EventStream {}

impl EventStream {
    pub(crate) fn new(device: Device) -> io::Result<Self> {
        use nix::fcntl;
        fcntl::fcntl(device.as_raw_fd(), fcntl::F_SETFL(fcntl::OFlag::O_NONBLOCK))
            .map_err(nix_err)?;
        let device = AsyncFd::new(device)?;
        Ok(Self { device })
    }

    /// Returns a reference to the underlying device
    pub fn device(&self) -> &Device {
        self.device.get_ref()
    }
}

impl Stream for EventStream {
    type Item = io::Result<InputEvent>;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let me = self.get_mut();

        if let Some(ev) = me.device.get_mut().pop_event() {
            return Poll::Ready(Some(Ok(ev)));
        }

        loop {
            let mut guard = ready!(me.device.poll_read_ready_mut(cx))?;

            match guard.try_io(|device| device.get_mut().fill_events(DEFAULT_EVENT_COUNT)) {
                Ok(res) => {
                    let ret = match res {
                        Ok(0) => None,
                        Ok(_) => Some(Ok(me.device.get_mut().pop_event().unwrap())),
                        Err(e) if e.raw_os_error() == Some(libc::ENODEV) => None,
                        Err(e) => Some(Err(e)),
                    };
                    return Poll::Ready(ret);
                }
                Err(_would_block) => continue,
            }
        }
    }
}
