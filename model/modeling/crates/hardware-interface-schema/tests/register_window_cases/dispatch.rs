use std::cell::Cell;
use std::error::Error as _;

use hardware_interface_schema::{RegisterAccessError as Error, RegisterWindow, register_dispatch};

use super::{BASE, Map};

#[test]
fn dispatch_evaluates_each_attempt_once_and_stops_on_success() {
    // Given
    let attempts = Cell::new(0);
    let attempt = |result| {
        attempts.set(attempts.get() + 1);
        result
    };
    // When
    let result: Result<u8, Error> = register_dispatch!(
        attempt(Err(Error::OutsideWindow { address: BASE })),
        attempt(Ok(7)),
        attempt(Ok(8)),
    );
    // Then
    assert_eq!(result, Ok(7));
    assert_eq!(attempts.get(), 2);
}

#[test]
fn dispatch_keeps_last_miss_and_accepts_singleton() {
    // Given / When
    let result: Result<(), Error> = register_dispatch!(
        Err(Error::OutsideWindow { address: 1 }),
        Err(Error::OutsideWindow { address: 2 }),
    );
    let single: Result<u8, Error> = register_dispatch!(Ok(3));
    // Then
    assert_eq!(result, Err(Error::OutsideWindow { address: 2 }));
    assert_eq!(single, Ok(3));
}

#[test]
fn dispatch_never_falls_through_terminal_validation_or_application_errors() {
    // Given
    let errors = [
        Error::InvalidOffset { offset: 1 },
        Error::WidthMismatch {
            offset: 0,
            expected: 2,
            actual: 1,
        },
        Error::ReadForbidden { offset: 8 },
        Error::WriteForbidden { offset: 4 },
        Error::ReservedBitChange {
            offset: 0,
            writable_mask: 1,
            changed: 2,
        },
        Error::InvalidLayout { offset: 0 },
        Error::Other("locked"),
    ];
    let fallbacks = Cell::new(0);
    for error in errors {
        // When
        let result: Result<(), Error<&str>> = register_dispatch!(Err(error), {
            fallbacks.set(fallbacks.get() + 1);
            Ok(())
        });
        // Then
        assert_eq!(result, Err(error));
        assert_eq!(fallbacks.get(), 0);
    }
}

#[test]
fn dispatch_mixes_plain_and_hooked_windows_with_one_mutable_buffer() {
    // Given
    let mut first = RegisterWindow::<Map>::new(0);
    let mut second = RegisterWindow::<Map>::new(BASE);
    let mut out = [0; 2];
    // When
    let result: Result<(), Error<&str>> = register_dispatch!(
        first.read(BASE, &mut out).map_err(Error::widen),
        second.read_with(BASE, &mut out, |_, _| Ok(()), |_, _| {}),
    );
    // Then
    assert_eq!(result, Ok(()));
    assert_eq!(out, [0xa1, 3]);
}

#[test]
fn error_mapping_preserves_schema_errors_and_exposes_custom_source() {
    // Given
    let original = Error::<u8>::Other(7);
    let plain = Error::InvalidOffset { offset: 4 };
    // When
    let mapped = original.map_other(u16::from);
    let widened: Error<u16> = plain.widen();
    let sourced = Error::Other(std::io::Error::other("locked"));
    // Then
    assert_eq!(mapped, Error::Other(7_u16));
    assert_eq!(widened, Error::InvalidOffset { offset: 4 });
    assert!(sourced.source().is_some());
    assert!(
        Error::<std::convert::Infallible>::OutsideWindow { address: 0 }
            .source()
            .is_none()
    );
}

#[test]
fn dispatch_finishes_before_its_result_returning_caller_continues() {
    fn caller() -> Result<u8, Error> {
        register_dispatch!(Err(Error::OutsideWindow { address: 1 }), Ok(()))?;
        Ok(9)
    }
    // Given / When
    let result = caller();
    // Then
    assert_eq!(result, Ok(9));
}

#[test]
fn reversing_disjoint_windows_preserves_the_owning_hook_and_bytes() {
    // Given
    let mut first = RegisterWindow::<Map>::new(0);
    let mut second = RegisterWindow::<Map>::new(BASE);
    let first_calls = Cell::new(0);
    let second_calls = Cell::new(0);
    let mut forward = [0; 2];
    let mut reverse = [0; 2];
    let first_hook = |_, _: &mut super::MapValues| {
        first_calls.set(first_calls.get() + 1);
        Ok::<_, ()>(())
    };
    let second_hook = |_, _: &mut super::MapValues| {
        second_calls.set(second_calls.get() + 1);
        Ok::<_, ()>(())
    };
    // When
    let forward_result = register_dispatch!(
        first.read_with(BASE, &mut forward, first_hook, |_, _| {}),
        second.read_with(BASE, &mut forward, second_hook, |_, _| {}),
    );
    let reverse_result = register_dispatch!(
        second.read_with(BASE, &mut reverse, second_hook, |_, _| {}),
        first.read_with(BASE, &mut reverse, first_hook, |_, _| {}),
    );
    // Then
    assert_eq!(forward_result, Ok(()));
    assert_eq!(reverse_result, forward_result);
    assert_eq!(forward, [0xa1, 3]);
    assert_eq!(reverse, forward);
    assert_eq!(first_calls.get(), 0);
    assert_eq!(second_calls.get(), 2);
}
