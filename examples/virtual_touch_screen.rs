use evdev::{
    uinput::VirtualDeviceBuilder, AbsInfo, AbsoluteAxisCode, AttributeSet, EventType, InputEvent,
};
use evdev::{KeyCode, KeyEvent, UinputAbsSetup};
use std::thread::sleep;
use std::time::Duration;

fn main() -> std::io::Result<()> {

    // Size of the touch screen
    let max_x = 1080;
    let max_y = 1920;

    let abs_setup_x = AbsInfo::new(0, 0, max_x, 0, 0, 0);
    let abs_setup_y = AbsInfo::new(0, 0, max_y, 0, 0, 0);

    // see https://www.kernel.org/doc/html/v4.17/input/event-codes.html
    let mut buttons = AttributeSet::<KeyCode>::new();
    buttons.insert(KeyCode::BTN_TOUCH);

    let mut device = VirtualDeviceBuilder::new()?
        .name("Fake TouchScreen")
        .with_keys(&buttons)?
        .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisCode::ABS_X, abs_setup_x))?
        .with_absolute_axis(&UinputAbsSetup::new(AbsoluteAxisCode::ABS_Y, abs_setup_y))?
        .build()?;

    for path in device.enumerate_dev_nodes_blocking()? {
        let path = path?;
        println!("Available as {}", path.display());
    }

    println!("Waiting for Ctrl-C...");

    // Emit some touch events
    for i in 0..10 {
        for j in 0..10 {
            let move_x = InputEvent::new_now(EventType::ABSOLUTE.0, AbsoluteAxisCode::ABS_X.0, i);
            let move_y = InputEvent::new_now(EventType::ABSOLUTE.0, AbsoluteAxisCode::ABS_Y.0, j);
            let down_event = *KeyEvent::new(KeyCode(KeyCode::BTN_TOUCH.0), 1);

            device.emit(&[down_event, move_x, move_y]).unwrap();
            println!("touching {i}, {j}");

            let up_event = *KeyEvent::new(KeyCode(KeyCode::BTN_TOUCH.0), 0);
            device.emit(&[up_event])?;
            println!("releasing {i}, {j}");
            sleep(Duration::from_millis(100));
        }
    }

    Ok(())
}
