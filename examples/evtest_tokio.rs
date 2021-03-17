//! Demonstrating how to monitor events with evdev + tokio

use tokio_1 as tokio;

// cli/"tui" shared between the evtest examples
mod _pick_device;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let d = _pick_device::pick_device();
    println!("{}", d);
    println!("Events:");
    let mut events = d.into_event_stream()?;
    loop {
        let ev = events.next_event().await?;
        println!("{:?}", ev);
    }
}
