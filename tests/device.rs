#![cfg(feature = "device-test")]

mod common;

use common::{get_test_device, key_event};
use evdev::KeyCode;

#[test]
pub fn test_get_key_state() -> Result<(), Box<dyn std::error::Error>> {
    let (input, mut output) = get_test_device()?;

    output.emit(&[key_event(KeyCode::KEY_DOT, 1)])?;

    assert_eq!(1, input.get_key_state()?.iter().count());
    assert!(input
        .get_key_state()?
        .iter()
        .all(|e| e.code() == KeyCode::KEY_DOT.code()));

    output.emit(&[key_event(KeyCode::KEY_DOT, 0)])?;

    assert_eq!(0, input.get_key_state()?.iter().count());

    Ok(())
}
