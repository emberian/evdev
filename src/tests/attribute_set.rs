use crate::{AttributeSet, KeyCode};

#[test]
pub fn test_iteration_keys() -> std::io::Result<()> {
    let mut keys: AttributeSet<KeyCode> = AttributeSet::new();

    for code in 1..59 {
        keys.insert(KeyCode::new(code));
    }

    assert_eq!(58, keys.iter().count());

    let mut expected: i16 = 58;

    for typ in keys.iter() {
        expected -= 1;

        assert_eq!(
            expected as usize,
            keys.slice_iter(KeyCode(typ.0 + 1)).count()
        );
    }

    Ok(())
}

#[test]
pub fn test_first_element_in_slice_iter() -> std::io::Result<()> {
    let mut keys: AttributeSet<KeyCode> = AttributeSet::new();
    keys.insert(KeyCode(0));
    keys.insert(KeyCode(1));

    assert_eq!(KeyCode(0), keys.slice_iter(KeyCode(0)).nth(0).unwrap());
    assert_eq!(KeyCode(1), keys.slice_iter(KeyCode(1)).nth(0).unwrap());

    Ok(())
}
