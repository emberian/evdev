// Create a virtual mouse, just while this is running.
// Generally this requires root.

use evdev::{uinput::VirtualDeviceBuilder, AttributeSet, EventType, InputEvent, RelativeAxisType};
use std::thread::sleep;
use std::time::Duration;
use MoveDirection::*;

fn main() -> std::io::Result<()> {
    let mut device = VirtualDeviceBuilder::new()?
        .name("fake-mouse")
        .with_relative_axes(&AttributeSet::from_iter([
            RelativeAxisType::REL_X,
            RelativeAxisType::REL_Y,
            RelativeAxisType::REL_WHEEL,
            RelativeAxisType::REL_HWHEEL,
        ]))?
        .build()
        .unwrap();

    for path in device.enumerate_dev_nodes_blocking()? {
        let path = path?;
        println!("Available as {}", path.display());
    }

    println!("Waiting for Ctrl-C...");
    loop {
        let ev = new_move_mouse_event(Up, 50);
        device.emit(&[ev]).unwrap();
        println!("Moved mouse up");
        sleep(Duration::from_millis(100));

        let ev = new_move_mouse_event(Down, 50);
        device.emit(&[ev]).unwrap();
        println!("Moved mouse down");
        sleep(Duration::from_millis(100));

        let ev = new_move_mouse_event(Left, 50);
        device.emit(&[ev]).unwrap();
        println!("Moved mouse left");
        sleep(Duration::from_millis(100));

        let ev = new_move_mouse_event(Right, 50);
        device.emit(&[ev]).unwrap();
        println!("Moved mouse right");
        sleep(Duration::from_millis(100));

        let ev = new_scroll_mouse_event(Up, 1);
        device.emit(&[ev]).unwrap();
        println!("Scrolled mouse up");
        sleep(Duration::from_millis(100));

        let ev = new_scroll_mouse_event(Down, 1);
        device.emit(&[ev]).unwrap();
        println!("Scrolled mouse down");
        sleep(Duration::from_millis(100));

        let ev = new_scroll_mouse_event(Left, 1);
        device.emit(&[ev]).unwrap();
        println!("Scrolled mouse left");
        sleep(Duration::from_millis(100));

        let ev = new_scroll_mouse_event(Right, 1);
        device.emit(&[ev]).unwrap();
        println!("Scrolled mouse right");
        sleep(Duration::from_millis(100));
    }
}

enum MoveDirection {
    Up,
    Down,
    Left,
    Right,
}

fn new_move_mouse_event(direction: MoveDirection, distance: u16) -> InputEvent {
    let (axis, distance) = match direction {
        MoveDirection::Up => (RelativeAxisType::REL_Y, -i32::from(distance)),
        MoveDirection::Down => (RelativeAxisType::REL_Y, i32::from(distance)),
        MoveDirection::Left => (RelativeAxisType::REL_X, -i32::from(distance)),
        MoveDirection::Right => (RelativeAxisType::REL_X, i32::from(distance)),
    };
    InputEvent::new_now(EventType::RELATIVE.0, axis.0, distance)
}

fn new_scroll_mouse_event(direction: MoveDirection, distance: u16) -> InputEvent {
    let (axis, distance) = match direction {
        MoveDirection::Up => (RelativeAxisType::REL_WHEEL.0, i32::from(distance)),
        MoveDirection::Down => (RelativeAxisType::REL_WHEEL.0, -i32::from(distance)),
        MoveDirection::Left => (RelativeAxisType::REL_HWHEEL.0, -i32::from(distance)),
        MoveDirection::Right => (RelativeAxisType::REL_HWHEEL.0, i32::from(distance)),
    };
    InputEvent::new_now(EventType::RELATIVE.0, axis, distance)
}
