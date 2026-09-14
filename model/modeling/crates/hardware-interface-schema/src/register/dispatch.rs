/// Tries register-window expressions lazily, falling through only outside a window.
///
/// Each attempted expression runs once. Success and every other error stop dispatch;
/// all misses return the final `OutsideWindow`. This is an expression and never
/// returns from the caller. Windows must not overlap; generated map placements
/// establish this precondition without probing accesses or running their effects.
///
/// All branches must use `Result<T, RegisterAccessError<E>>`. A plain window access
/// can join hooked accesses with `.map_err(RegisterAccessError::widen)`.
///
/// ```
/// use hardware_interface_schema::{RegisterAccessError, register_dispatch};
/// let result: Result<u8, RegisterAccessError> = register_dispatch!(
///     Err(RegisterAccessError::OutsideWindow { address: 16 }), Ok(7),
/// );
/// assert_eq!(result, Ok(7));
/// ```
///
/// Other error families are rejected even for a single branch:
///
/// ```compile_fail
/// use hardware_interface_schema::register_dispatch;
/// let _: Result<(), &str> = register_dispatch!(Err("wrong family"));
/// ```
#[macro_export]
macro_rules! register_dispatch {
    ($last:expr $(,)?) => {{
        let result: ::core::result::Result<_, $crate::RegisterAccessError<_>> = $last;
        result
    }};
    ($first:expr, $($remaining:expr),+ $(,)?) => {{
        match $first {
            ::core::result::Result::Err($crate::RegisterAccessError::OutsideWindow { .. }) => {
                $crate::register_dispatch!($($remaining),+)
            }
            result => result,
        }
    }};
}
