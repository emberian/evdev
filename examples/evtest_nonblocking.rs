//! This example demonstrates how to use the evdev crate with a nonblocking file descriptor.
//!
//! Note that for this implementation the caller is responsible for ensuring the underlying
//! Device file descriptor is set to O_NONBLOCK. The caller must also create the epoll descriptor,
//! bind it, check for EAGAIN returns from fetch_events_*, call epoll_wait as appropriate, and
//! clean up the epoll file descriptor when finished.

#[cfg(not(target_os = "linux"))]
fn main() {}

// cli/"tui" shared between the evtest examples
#[cfg(target_os = "linux")]
mod _pick_device;

#[cfg(target_os = "linux")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use nix::{
        fcntl::{FcntlArg, OFlag},
        sys::epoll,
    };
    use std::os::unix::io::AsRawFd;

    let mut d = _pick_device::pick_device();
    println!("{}", d);

    let raw_fd = d.as_raw_fd();
    // Set nonblocking
    nix::fcntl::fcntl(raw_fd, FcntlArg::F_SETFL(OFlag::O_NONBLOCK))?;

    // Create epoll handle and attach raw_fd
    let epoll_fd = crate::epoll::Epoll::new(epoll::epoll_create1(
        epoll::EpollCreateFlags::EPOLL_CLOEXEC,
    )?);
    let mut event = epoll::EpollEvent::new(epoll::EpollFlags::EPOLLIN, 0);
    epoll::epoll_ctl(
        epoll_fd.as_raw_fd(),
        epoll::EpollOp::EpollCtlAdd,
        raw_fd,
        Some(&mut event),
    )?;

    // We don't care about these, but the kernel wants to fill them.
    let mut events = [epoll::EpollEvent::empty(); 2];

    println!("Events:");
    loop {
        match d.fetch_events() {
            Ok(iterator) => {
                for ev in iterator {
                    println!("{:?}", ev);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // Wait forever for bytes available on raw_fd
                epoll::epoll_wait(epoll_fd.as_raw_fd(), &mut events, -1)?;
            }
            Err(e) => {
                eprintln!("{}", e);
                break;
            }
        }
    }
    Ok(())
}

#[cfg(target_os = "linux")]
mod epoll {
    use std::os::unix::io::{AsRawFd, RawFd};

    // The rest here is to ensure the epoll handle is cleaned up properly.
    // You can also use the epoll crate, if you prefer.
    pub(crate) struct Epoll(RawFd);

    impl Epoll {
        pub(crate) fn new(fd: RawFd) -> Self {
            Epoll(fd)
        }
    }

    impl AsRawFd for Epoll {
        fn as_raw_fd(&self) -> RawFd {
            self.0
        }
    }

    impl Drop for Epoll {
        fn drop(&mut self) {
            let _ = nix::unistd::close(self.0);
        }
    }
}
