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
fn main() -> std::io::Result<()> {
    use nix::sys::epoll;

    let mut dev = _pick_device::pick_device();
    println!("{dev}");

    dev.set_nonblocking(true)?;

    // Create epoll handle and attach raw_fd
    let epoll = epoll::Epoll::new(epoll::EpollCreateFlags::EPOLL_CLOEXEC)?;
    let event = epoll::EpollEvent::new(epoll::EpollFlags::EPOLLIN, 0);
    epoll.add(&dev, event)?;

    // We don't care about these, but the kernel wants to fill them.
    let mut events = [epoll::EpollEvent::empty(); 2];

    println!("Events:");
    loop {
        match dev.fetch_events() {
            Ok(iterator) => {
                for ev in iterator {
                    println!("{ev:?}");
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // Wait forever for bytes available on dev
                epoll.wait(&mut events, epoll::EpollTimeout::NONE)?;
            }
            Err(e) => {
                eprintln!("{e}");
                break;
            }
        }
    }
    Ok(())
}
