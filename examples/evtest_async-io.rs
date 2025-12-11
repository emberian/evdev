//! Demonstrating how to monitor events with evdev + async-io

// cli/"tui" shared between the evtest examples
mod _pick_device;

fn main() {
    let d = _pick_device::pick_device();
    println!("{}", d);
    println!("Events:");
    let mut events = d.into_event_stream().unwrap();
    futures_lite::future::block_on(async {
        loop {
            let ev = events.next_event().await.unwrap();
            println!("{:?}", ev);
        }
    });
}
