//! Closure-scoped generic storage policies.
//!
//! A backend call linearizes while its closure owns the only active view of the
//! stored value. The higher-ranked closure lifetime prevents that view, a
//! guard-like mutable reference, or a future borrowing either reference from
//! escaping the call. Closures are synchronous and cannot suspend.
//!
//! [`LocalStorage`] is clone-shared within one thread and is neither `Send` nor
//! `Sync`. [`MutexStorage`] is clone-shared across threads and is `Send` and
//! `Sync` when `T: Send`. Ordinary cross-thread contention blocks and
//! serializes. Any nested call through a clone of the same backend on the same
//! thread returns [`StorageError::Reentrant`] before its closure runs; calls to
//! a different backend may be nested.

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
/// use hardware_interface_runtime::{LocalStorage, StorageBackend};
/// let storage = LocalStorage::new(String::from("value"));
/// let escaped = storage.inspect(String::as_str);
/// drop(storage);
/// println!("{escaped:?}");
/// ```
///
/// Guard-like exclusive references cannot escape:
///
/// ```compile_fail
/// use hardware_interface_runtime::{LocalStorage, StorageBackend};
/// let storage = LocalStorage::new(1_u64);
/// let escaped = storage.mutate(|value| value);
/// drop(storage);
/// println!("{escaped:?}");
/// ```
///
/// A future borrowing the value cannot escape:
///
/// ```compile_fail
/// use hardware_interface_runtime::{LocalStorage, StorageBackend};
/// let storage = LocalStorage::new(1_u64);
/// let future = storage.inspect(|value| async move { *value });
/// drop(storage);
/// drop(future);
/// ```
///
/// Async closures are rejected because backend operations are synchronous:
///
/// ```compile_fail
/// use hardware_interface_runtime::{LocalStorage, StorageBackend};
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
    /// # Errors
    /// Returns [`StorageError::Reentrant`] before `operation` runs when the
    /// current thread already has an active call on the same backend.
    fn mutate<R>(
        &self,
        operation: impl for<'value> FnOnce(&'value mut T) -> R,
    ) -> Result<R, StorageError>;
}
