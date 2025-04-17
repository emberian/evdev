#![allow(dead_code)]

use evdev::uinput::VirtualDevice;
use evdev::{AttributeSet, BusType, Device, EventType, InputEvent, InputId, KeyCode, SwitchCode};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

pub fn key_click(key: KeyCode) -> Vec<InputEvent> {
    vec![key_event(key, 1), key_event(key, 0)]
}

pub fn key_event(key: KeyCode, key_state: i32) -> InputEvent {
    InputEvent::new(EventType::KEY.0, key.code(), key_state)
}

pub fn get_test_device() -> std::io::Result<(Device, VirtualDevice)> {
    let (name, output) = get_device()?;

    let mut input = Device::open(&name)?;

    input.grab()?;

    Ok((input, output))
}

pub fn get_device() -> std::io::Result<(PathBuf, VirtualDevice)> {
    let mut keys: AttributeSet<KeyCode> = AttributeSet::new();
    for code in 1..59 {
        let key = KeyCode::new(code);
        let name = format!("{:?}", key);
        if name.starts_with("KEY_") {
            keys.insert(key);
        }
    }

    let mut sw: AttributeSet<SwitchCode> = AttributeSet::new();

    sw.insert(SwitchCode::SW_LID);
    sw.insert(SwitchCode::SW_TABLET_MODE);

    let mut device = VirtualDevice::builder()?
        .input_id(InputId::new(BusType::BUS_USB, 0x1234, 0x5678, 0x111))
        .name("test device")
        .with_keys(&keys)?
        .with_switches(&sw)?
        .build()?;

    // Fetch name.
    let d: Vec<std::path::PathBuf> = device
        .enumerate_dev_nodes_blocking()?
        .map(|p| p.unwrap())
        .collect();

    thread::sleep(Duration::from_millis(100)); // To avoid permission denied.

    Ok((d.first().unwrap().clone(), device))
}

pub fn final_dot_state(start_state: i32, events: impl Iterator<Item = InputEvent>) -> i32 {
    events.fold(start_state, |state, ev| {
        if ev.event_type() == EventType::KEY && ev.code() == KeyCode::KEY_DOT.code() {
            if ev.value() == 0 {
                0
            } else {
                1
            }
        } else {
            state
        }
    })
}

pub fn final_event_state(key: KeyCode, events: &Vec<InputEvent>) -> Option<i32> {
    events.iter().fold(None, |state, ev| {
        if ev.event_type() == EventType::KEY && ev.code() == key.code() {
            if ev.value() == 0 {
                Some(0)
            } else {
                Some(1)
            }
        } else {
            state
        }
    })
}
