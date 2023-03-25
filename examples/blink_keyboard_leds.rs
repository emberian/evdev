use evdev::{LedEvent, LedType};

mod _pick_device;

fn main() {
    let mut d = _pick_device::pick_device();
    println!("{d}");
    println!("Blinking the Keyboard LEDS...");
    for i in 0..5 {
        let on = i % 2 != 0;
        d.send_events(&[
            LedEvent::new(LedType::LED_CAPSL, if on { i32::MAX } else { 0 }),
            LedEvent::new(LedType::LED_NUML, if on { i32::MAX } else { 0 }),
            LedEvent::new(LedType::LED_SCROLLL, if on { i32::MAX } else { 0 }),
        ])
        .unwrap();
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
