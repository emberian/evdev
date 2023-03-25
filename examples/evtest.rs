//! Similar to the evtest tool.

// cli/"tui" shared between the evtest examples
mod _pick_device;

fn main() {
    let mut d = _pick_device::pick_device();
    println!("{d}");
    println!("Events:");
    loop {
        for ev in d.fetch_events().unwrap() {
            println!("{ev:?}");
        }
    }
}
