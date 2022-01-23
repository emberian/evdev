//use evdev::{Device};
use std::error::Error;
use std::thread::sleep;
use std::time::Duration;

use tokio_1 as tokio;
use tokio::time::timeout;

use evdev::{uinput::VirtualDeviceBuilder, AttributeSet, EventType, InputEvent, Key};

#[tokio::test]
async fn test_virtual_device_actually_emits() -> Result<(), Box<dyn Error>> {
    let mut keys = AttributeSet::<Key>::new();
    let virtual_device_name = "fake-keyboard";
    keys.insert(Key::KEY_ESC);

    let mut device = VirtualDeviceBuilder::new()?
        .name(virtual_device_name)
        .with_keys(&keys)?
        .build()
        .unwrap();

    let mut maybe_device = None;
    sleep(Duration::from_millis(500));
    for (_i, d) in evdev::enumerate() {
        println!("{:?}", d.name());
        if d.name() == Some(virtual_device_name) {
            maybe_device = Some(d);
            break
        }
    }
    assert_eq!(maybe_device.is_some(), true);
    let listen_device = maybe_device.unwrap();

    let type_ = EventType::KEY;
    let code = Key::BTN_DPAD_UP.code();

    // listen for events on the listen device
    let listener = tokio::spawn(async move {
        // try to read the key code that will be sent through virtual device
        let mut events = listen_device.into_event_stream()?;
        events.next_event().await
    });

    // emit a key code through virtual device
    let down_event = InputEvent::new(type_, code, 10);
    device.emit(&[down_event]).unwrap();

    let event = timeout(Duration::from_secs(1), listener).await???;

    assert_eq!(down_event.event_type(), event.event_type());
    assert_eq!(down_event.code(), event.code());

    // wait for listener
    Ok(())
}
