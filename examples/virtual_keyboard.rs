// Create a virtual keyboard, just while this is running.
// Generally this requires root.

use evdev::KeyEvent;
use evdev::{uinput::VirtualDeviceBuilder, AttributeSet, EventType, InputEvent, KeyType};
use std::thread::sleep;
use std::time::Duration;

fn main() -> std::io::Result<()> {
    let mut keys = AttributeSet::<KeyType>::new();
    keys.insert(KeyType::BTN_DPAD_UP);

    let mut device = VirtualDeviceBuilder::new()?
        .name("Fake Keyboard")
        .with_keys(&keys)?
        .build()
        .unwrap();

    for path in device.enumerate_dev_nodes_blocking()? {
        let path = path?;
        println!("Available as {}", path.display());
    }

    // Note this will ACTUALLY PRESS the button on your computer.
    // Hopefully you don't have BTN_DPAD_UP bound to anything important.
    let code = KeyType::BTN_DPAD_UP.code();

    println!("Waiting for Ctrl-C...");
    loop {
        // this guarantees a key event
        let down_event = *KeyEvent::new(KeyType(code), 1);
        device.emit(&[down_event]).unwrap();
        println!("Pressed.");
        sleep(Duration::from_secs(2));

        // alternativeley we can create a InputEvent, which will be any variant of InputEvent
        // depending on the type_ value
        let up_event = InputEvent::new(EventType::KEY.0, code, 0);
        device.emit(&[up_event]).unwrap();
        println!("Released.");
        sleep(Duration::from_secs(2));
    }
}
