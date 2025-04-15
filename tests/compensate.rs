#![cfg(feature = "device-test")]

mod common;

use common::{final_dot_state, final_event_state, get_test_device, key_click, key_event};
use evdev::{EventType, InputEvent, KeyCode, SwitchCode};

#[test]
pub fn test_compensate_keys() -> std::io::Result<()> {
    let (mut input, mut output) = get_test_device()?;

    let dot_state: i32 = 0;

    // Make overflow.
    for _ in 0..30 {
        output.emit(&key_click(KeyCode::KEY_DOT))?;
    }

    let dot_state = final_dot_state(dot_state, input.fetch_events()?);

    assert_eq!(0, dot_state);

    // Press and release.
    output.emit(&key_click(KeyCode::KEY_DOT))?;

    let dot_state = final_dot_state(dot_state, input.fetch_events()?);

    assert_eq!(0, dot_state);

    // Just press.
    output.emit(&[key_event(KeyCode::KEY_DOT, 1)])?;

    let dot_state = final_dot_state(dot_state, input.fetch_events()?);

    assert_eq!(1, dot_state);

    Ok(())
}

#[test]
pub fn test_compensate_with_key_down() -> std::io::Result<()> {
    let (mut input, mut output) = get_test_device()?;

    output.emit(&[InputEvent::new(EventType::KEY.0, KeyCode::KEY_A.0, 1)])?;
    output.emit(&[InputEvent::new(EventType::KEY.0, KeyCode::KEY_B.0, 1)])?;

    // Make overflow.
    for _ in 0..30 {
        output.emit(&key_click(KeyCode::KEY_DOT))?;
    }

    assert_eq!(0, input.fetch_events()?.into_iter().count());

    // Press and release.
    output.emit(&key_click(KeyCode::KEY_DOT))?;

    let events = input
        .fetch_events()?
        .into_iter()
        .collect::<Vec<InputEvent>>();

    assert_eq!(Some(1), final_event_state(KeyCode::KEY_A, &events));
    assert_eq!(Some(1), final_event_state(KeyCode::KEY_B, &events));

    Ok(())
}

#[test]
pub fn test_compensate_with_switch_down() -> std::io::Result<()> {
    let (mut input, mut output) = get_test_device()?;

    let dot_state: i32 = 0;

    output.emit(&[InputEvent::new(
        EventType::SWITCH.0,
        SwitchCode::SW_LID.0,
        1,
    )])?;

    // Make overflow.
    for _ in 0..30 {
        output.emit(&key_click(KeyCode::KEY_DOT))?;
    }

    let dot_state = final_dot_state(dot_state, input.fetch_events()?);

    assert_eq!(0, dot_state);

    // Press and release.
    output.emit(&key_click(KeyCode::KEY_DOT))?;

    let dot_state = final_dot_state(dot_state, input.fetch_events()?);

    assert_eq!(0, dot_state);

    Ok(())
}
