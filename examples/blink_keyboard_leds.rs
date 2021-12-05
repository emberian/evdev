use evdev::{EventType, InputEvent, LedType};

mod _pick_device;

fn main() {
    let mut d = _pick_device::pick_device();
    println!("{}", d);
    println!("Blinking the Keyboard LEDS...");
    for i in 0..5 {
        let on = i % 2 != 0;
        d.send_event(&InputEvent::new(
            EventType::LED,
            LedType::LED_CAPSL.0,
            if on { i32::MAX } else { 0 },
        ))
        .unwrap();
        d.send_event(&InputEvent::new(
            EventType::LED,
            LedType::LED_NUML.0,
            if on { i32::MAX } else { 0 },
        ))
        .unwrap();
        d.send_event(&InputEvent::new(
            EventType::LED,
            LedType::LED_SCROLLL.0,
            if on { i32::MAX } else { 0 },
        ))
        .unwrap();
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
