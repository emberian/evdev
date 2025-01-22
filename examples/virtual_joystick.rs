// Create a virtual joystick, just while this is running.
// Generally this requires root.

use evdev::{uinput::VirtualDevice, AbsInfo, AbsoluteAxisCode, AbsoluteAxisEvent, UinputAbsSetup};
use std::thread::sleep;
use std::time::Duration;

fn main() -> std::io::Result<()> {
    let abs_setup = AbsInfo::new(256, 0, 512, 20, 20, 1);
    let abs_x = UinputAbsSetup::new(AbsoluteAxisCode::ABS_X, abs_setup);

    let mut device = VirtualDevice::builder()?
        .name("Fake Joystick")
        .with_absolute_axis(&abs_x)?
        .build()
        .unwrap();

    for path in device.enumerate_dev_nodes_blocking()? {
        let path = path?;
        println!("Available as {}", path.display());
    }

    // Hopefully you don't have ABS_X bound to anything important.
    let code = AbsoluteAxisCode::ABS_X.0;

    println!("Waiting for Ctrl-C...");
    loop {
        let down_event = *AbsoluteAxisEvent::new(AbsoluteAxisCode(code), 0);
        device.emit(&[down_event]).unwrap();
        println!("Minned out.");
        sleep(Duration::from_secs(2));

        let up_event = *AbsoluteAxisEvent::new(AbsoluteAxisCode(code), 512);
        device.emit(&[up_event]).unwrap();
        println!("Maxed out.");
        sleep(Duration::from_secs(2));
    }
}
