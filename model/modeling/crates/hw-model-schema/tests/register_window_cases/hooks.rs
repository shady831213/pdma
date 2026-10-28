use std::cell::{Cell, RefCell};

use hw_model_schema::{RegisterAccessError as Error, RegisterWindow};

use super::{BASE, ControlValues, Map};

#[test]
fn read_hooks_refresh_then_clear_after_serialization() -> Result<(), Error<&'static str>> {
    // Given
    let mut window = RegisterWindow::<Map>::new(BASE);
    let events = RefCell::new(Vec::new());
    let mut out = [0; 2];
    // When
    window.read_with(
        BASE,
        &mut out,
        |offset, next| {
            events.borrow_mut().push(("before", offset));
            next.control = ControlValues::new(0xb207);
            Ok(())
        },
        |offset, next| {
            events.borrow_mut().push(("after", offset));
            next.control = ControlValues::new(0);
        },
    )?;
    // Then
    assert_eq!(out, [0xb2, 7]);
    assert_eq!(window.values.control.raw(), 0);
    assert_eq!(*events.borrow(), [("before", 0), ("after", 0)]);
    Ok(())
}

#[test]
fn rejected_read_hook_rolls_back_values_and_destination() {
    // Given
    let mut window = RegisterWindow::<Map>::new(BASE);
    let before = window.values;
    let mut out = [0x5a; 2];
    let after_called = Cell::new(false);
    // When
    let result = window.read_with(
        BASE,
        &mut out,
        |_, next| {
            next.control = ControlValues::new(0);
            Err("busy")
        },
        |_, _| after_called.set(true),
    );
    // Then
    assert_eq!(result, Err(Error::Other("busy")));
    assert_eq!(window.values, before);
    assert_eq!(out, [0x5a; 2]);
    assert!(!after_called.get());
}

#[test]
fn write_hook_compares_current_and_candidate_before_commit() -> Result<(), Error<&'static str>> {
    // Given
    let mut window = RegisterWindow::<Map>::new(BASE);
    let events = RefCell::new(Vec::new());
    // When
    window.write_with(
        BASE,
        &[0xb2, 7],
        |offset, old, next| {
            events
                .borrow_mut()
                .push((offset, old.control.raw(), next.control.raw()));
            Ok(())
        },
        |offset, next| {
            events.borrow_mut().push((offset, next.control.raw(), 0));
            next.control = ControlValues::new(0);
        },
    )?;
    // Then
    assert_eq!(*events.borrow(), [(0, 0xa103, 0xb207), (0, 0xb207, 0)]);
    assert_eq!(window.values.control.raw(), 0);
    Ok(())
}

#[test]
fn rejected_write_hook_preserves_complete_state() {
    // Given
    let mut window = RegisterWindow::<Map>::new(BASE);
    let before = window.values;
    let after_called = Cell::new(false);
    // When
    let result = window.write_with(
        BASE,
        &[0xb2, 7],
        |_, _, _| Err("locked"),
        |_, _| after_called.set(true),
    );
    // Then
    assert_eq!(result, Err(Error::Other("locked")));
    assert_eq!(window.values, before);
    assert!(!after_called.get());
}

#[test]
fn invalid_access_never_invokes_hooks() {
    // Given
    let mut window = RegisterWindow::<Map>::new(BASE);
    let before = window.values;
    let calls = Cell::new(0);
    for (address, bytes) in [
        (BASE - 1, &[0; 2][..]),
        (BASE + 1, &[0; 2][..]),
        (BASE, &[0][..]),
        (BASE + 4, &[0; 4][..]),
        (BASE, &[0xa1, 0x13][..]),
    ] {
        // When
        let result = window.write_with::<()>(
            address,
            bytes,
            |_, _, _| {
                calls.set(calls.get() + 1);
                Ok(())
            },
            |_, _| calls.set(calls.get() + 1),
        );
        // Then
        assert!(result.is_err());
        assert_eq!(window.values, before);
        assert_eq!(calls.get(), 0);
    }
    for (address, width) in [(BASE - 1, 2), (BASE + 1, 2), (BASE, 1), (BASE + 8, 1)] {
        let mut out = vec![0x5a; width];
        // When
        let result = window.read_with::<()>(
            address,
            &mut out,
            |_, _| {
                calls.set(calls.get() + 1);
                Ok(())
            },
            |_, _| calls.set(calls.get() + 1),
        );
        // Then
        assert!(result.is_err());
        assert_eq!(out, vec![0x5a; width]);
        assert_eq!(calls.get(), 0);
    }
}
