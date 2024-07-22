// Create a virtual keyboard, just while this is running.
// Generally this requires root.

use evdev::{
    uinput::VirtualDeviceBuilder, AttributeSet, EventType, InputEvent, KeyCode, LedCode,
};
use std::thread::sleep;
use std::time::Duration;

fn main() -> std::io::Result<()> {
    let mut keys = AttributeSet::<evdev::KeyCode>::new();
    keys.insert(KeyCode::KEY_CAPSLOCK);
    keys.insert(KeyCode::KEY_SCROLLLOCK);

    let mut leds = AttributeSet::<evdev::LedCode>::new();
    leds.insert(LedCode::LED_CAPSL);
    leds.insert(LedCode::LED_SCROLLL);

    let mut device = VirtualDeviceBuilder::new()?
        .name("Fake Keyboard")
        .with_keys(&keys)?
        .with_leds(&leds)?
        .build()
        .unwrap();

    for path in device.enumerate_dev_nodes_blocking()? {
        let path = path?;
        println!("Available as {}", path.display());
    }

    println!("Blinking the Virtual Keyboard LEDS...");
    for _ in 0..4 {
        let capslock_down = InputEvent::new(EventType::KEY.0, KeyCode::KEY_CAPSLOCK.code(), 1);
        let capslock_up = InputEvent::new(EventType::KEY.0, KeyCode::KEY_CAPSLOCK.code(), 0);
        device.emit(&[capslock_down, capslock_up])?;
        sleep(Duration::from_millis(300));
        println!(
            "Capslock clicked, get_key_state: {:?}",
            device.get_led_state()
        );
        sleep(Duration::from_secs(2));
    }
    Ok(())
}
