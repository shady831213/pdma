use hardware_interface_schema::{RegisterAccessError as Error, RegisterWindow};
use std::cell::Cell;

use super::{BASE, ControlValues, Map, MapValues};

#[test]
fn nonzero_base_preserves_endian_and_authoritative_values() -> Result<(), Error> {
    // Given
    const WINDOW: RegisterWindow<Map> = RegisterWindow::new(BASE);
    let mut window = WINDOW;
    assert_eq!(window.base(), BASE);
    assert_eq!(window.values, MapValues::default());
    window.values.control = ControlValues::new(0xb207);
    let mut bytes = [0; 2];
    // When
    window.read(BASE, &mut bytes)?;
    window.write(BASE, &[0xc3, 0x09])?;
    // Then
    assert_eq!(bytes, [0xb2, 0x07]);
    assert_eq!(window.values.control.raw(), 0xc309);
    Ok(())
}

#[test]
fn outside_addresses_differ_from_interiors_and_padding() {
    // Given
    let mut window = RegisterWindow::<Map>::new(BASE);
    let cases = [
        (BASE - 1, Error::OutsideWindow { address: BASE - 1 }),
        (BASE + 16, Error::OutsideWindow { address: BASE + 16 }),
        (
            usize::MAX,
            Error::OutsideWindow {
                address: usize::MAX,
            },
        ),
        (BASE + 1, Error::InvalidOffset { offset: 1 }),
        (BASE + 2, Error::InvalidOffset { offset: 2 }),
        (BASE + 15, Error::InvalidOffset { offset: 15 }),
    ];
    for (address, expected) in cases {
        let mut out = [0x5a; 2];
        let before = window.values;
        // When
        let read = window.read(address, &mut out);
        let write = window.write(address, &[0; 2]);
        // Then
        assert_eq!(read, Err(expected));
        assert_eq!(write, Err(expected));
        assert_eq!(out, [0x5a; 2]);
        assert_eq!(window.values, before);
    }
}

#[test]
fn high_window_base_never_wraps() -> Result<(), Error> {
    // Given
    let mut window = RegisterWindow::<Map>::new(usize::MAX - 4);
    let mut bytes = [0; 2];
    // When
    window.read(usize::MAX - 4, &mut bytes)?;
    let below = window.read(0, &mut bytes);
    // Then
    assert_eq!(bytes, [0xa1, 3]);
    assert_eq!(below, Err(Error::OutsideWindow { address: 0 }));
    Ok(())
}

#[test]
fn rejected_writes_preserve_the_complete_value() {
    // Given
    let mut window = RegisterWindow::<Map>::new(BASE);
    let before = window.values;
    let cases: &[(usize, &[u8], Error)] = &[
        (
            BASE,
            &[0],
            Error::WidthMismatch {
                offset: 0,
                expected: 2,
                actual: 1,
            },
        ),
        (BASE + 4, &[0; 4], Error::WriteForbidden { offset: 4 }),
        (
            BASE,
            &[0xa1, 0x13],
            Error::ReservedBitChange {
                offset: 0,
                writable_mask: 0xff0f,
                changed: 0x10,
            },
        ),
    ];
    for (address, bytes, expected) in cases {
        // When
        let result = window.write(*address, bytes);
        // Then
        assert_eq!(result, Err(*expected));
        assert_eq!(window.values, before);
    }
}

#[test]
fn forbidden_read_preserves_destination() {
    // Given
    let mut window = RegisterWindow::<Map>::new(BASE);
    let mut out = [0x5a];
    // When
    let result = window.read(BASE + 8, &mut out);
    // Then
    assert_eq!(result, Err(Error::ReadForbidden { offset: 8 }));
    assert_eq!(out, [0x5a]);
}

#[test]
fn overflowing_transaction_is_terminal_before_hooks_or_mutation() {
    // Given
    let mut window = RegisterWindow::<Map>::new(usize::MAX);
    let before = window.values;
    let mut out = [0x5a; 2];
    let calls = Cell::new(0);
    // When
    let read = window.read_with::<()>(
        usize::MAX,
        &mut out,
        |_, _| {
            calls.set(calls.get() + 1);
            Ok(())
        },
        |_, _| calls.set(calls.get() + 1),
    );
    let write = window.write_with::<()>(
        usize::MAX,
        &[0xb2, 7],
        |_, _, _| {
            calls.set(calls.get() + 1);
            Ok(())
        },
        |_, _| calls.set(calls.get() + 1),
    );
    // Then
    assert_eq!(read, Err(Error::InvalidLayout { offset: 0 }));
    assert_eq!(write, Err(Error::InvalidLayout { offset: 0 }));
    assert_eq!(window.values, before);
    assert_eq!(out, [0x5a; 2]);
    assert_eq!(calls.get(), 0);
}
