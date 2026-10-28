#![doc = include_str!("storage/README.md")]

mod local;
mod mutex;

pub use local::LocalStorage;
pub use mutex::MutexStorage;

use std::error::Error;
use std::fmt::{Display, Formatter};

/// Failure from a closure-scoped storage operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageError {
    /// The current thread already has an active call on this backend.
    Reentrant,
}

impl Display for StorageError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Reentrant => formatter.write_str("storage backend call is reentrant"),
        }
    }
}

impl Error for StorageError {}

/// A synchronous backend exposing only closure-scoped value access.
///
/// Each successful call is one atomic mutation or inspection relative to all
/// other calls on the same backend. `R` cannot borrow the supplied value
/// because it is independent of the closure's higher-ranked input lifetime.
///
/// Borrowed inspection results cannot escape:
///
/// ```compile_fail
/// use hw_model_io::memory::storage::{LocalStorage, StorageBackend};
/// let storage = LocalStorage::new(String::from("value"));
/// let escaped = storage.inspect(String::as_str);
/// drop(storage);
/// println!("{escaped:?}");
/// ```
///
/// Guard-like exclusive references cannot escape:
///
/// ```compile_fail
/// use hw_model_io::memory::storage::{LocalStorage, StorageBackend};
/// let storage = LocalStorage::new(1_u64);
/// let escaped = storage.mutate(|value| value);
/// drop(storage);
/// println!("{escaped:?}");
/// ```
///
/// A future borrowing the value cannot escape:
///
/// ```compile_fail
/// use hw_model_io::memory::storage::{LocalStorage, StorageBackend};
/// let storage = LocalStorage::new(1_u64);
/// let future = storage.inspect(|value| async move { *value });
/// drop(storage);
/// drop(future);
/// ```
///
/// Async closures are rejected because backend operations are synchronous:
///
/// ```compile_fail
/// use hw_model_io::memory::storage::{LocalStorage, StorageBackend};
/// let storage = LocalStorage::new(1_u64);
/// let future = storage.mutate(async |value| *value += 1);
/// drop(storage);
/// drop(future);
/// ```
pub trait StorageBackend<T> {
    /// Inspects the stored value during one linearized call.
    ///
    /// # Errors
    /// Returns [`StorageError::Reentrant`] when the current thread already has
    /// an active call on the same backend.
    fn inspect<R>(
        &self,
        operation: impl for<'value> FnOnce(&'value T) -> R,
    ) -> Result<R, StorageError>;

    /// Mutates the stored value during one linearized call.
    ///
    /// Mutation is in place. An application `Result` is returned as `R`, so an
    /// application error does not undo writes. Callback panics also do not provide
    /// rollback; the supplied backends retain the mutated value after unwinding.
    ///
    /// # Errors
    /// Returns [`StorageError::Reentrant`] before `operation` runs when the
    /// current thread already has an active call on the same backend.
    fn mutate<R>(
        &self,
        operation: impl for<'value> FnOnce(&'value mut T) -> R,
    ) -> Result<R, StorageError>;
}
