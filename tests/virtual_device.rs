#![cfg(any(feature = "tokio", feature = "async-io"))]

use std::error::Error;
use std::thread::sleep;
use std::time::Duration;

#[cfg(feature = "tokio")]
use tokio::time::timeout;
#[cfg(feature = "async-io")]
use futures_lite::FutureExt;

use evdev::{uinput::VirtualDevice, AttributeSet, EventType, InputEvent, KeyCode};

#[test]
fn test_virtual_device_actually_emits() -> Result<(), Box<dyn Error>> {
    #[cfg(feature = "async-io")]
    let ex = async_executor::Executor::new();

    let fut = async {
        let mut keys = AttributeSet::<KeyCode>::new();
        let virtual_device_name = "fake-keyboard";
        keys.insert(KeyCode::KEY_ESC);

        let mut device = VirtualDevice::builder()?
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
                break;
            }
        }
        assert!(maybe_device.is_some());
        let listen_device = maybe_device.unwrap();

        let type_ = EventType::KEY;
        let code = KeyCode::KEY_ESC.code();

        let fut = async move {
            // try to read the key code that will be sent through virtual device
            let mut events = listen_device.into_event_stream()?;
            events.next_event().await
        };

        // listen for events on the listen device
        #[cfg(feature = "tokio")]
        let listener = tokio::spawn(fut);
        #[cfg(feature = "async-io")]
        let listener = ex.spawn(fut);

        // emit a key code through virtual device
        let down_event = InputEvent::new(type_.0, code, 10);
        device.emit(&[down_event]).unwrap();

        let time = Duration::from_secs(1);
        #[cfg(feature = "tokio")]
        let event = timeout(time, listener).await???;
        #[cfg(feature = "async-io")]
        let event = listener.or(async {
            async_io::Timer::after(time).await;
            Err(std::io::ErrorKind::TimedOut.into())
        }).await?;

        assert_eq!(down_event.event_type(), event.event_type());
        assert_eq!(down_event.code(), event.code());

        // wait for listener
        Ok(())
    };

    #[cfg(feature = "tokio")]
    let res = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(fut);
    #[cfg(feature = "async-io")]
    let res = futures_lite::future::block_on(fut);

    res
}
