//! Clone sharing, application errors and same-backend reentry.
//!
//! Run: `cargo run -p hw-model-io --example callback_result`.
//! `LocalStorage` suits one-thread use; `MutexStorage` applies the same callback
//! contract to synchronized access across threads when the value is Send.
use hw_model_io::memory::storage::{LocalStorage, StorageBackend, StorageError};

fn main() {
    let storage = LocalStorage::new(0);
    let clone = storage.clone();
    // The outer Result describes access acquisition. The callback's Result is
    // returned unchanged inside it: this in-place mutation is not a transaction.
    let result = storage.mutate(|value| {
        *value = 7;
        Err::<(), _>("application failure")
    });
    assert_eq!(result, Ok(Err("application failure")));
    assert_eq!(clone.inspect(|value| *value), Ok(7));
    // Clones alias one value, so nested access through a clone is still reentry.
    assert_eq!(
        storage.inspect(|_| clone.inspect(|value| *value)),
        Ok(Err(StorageError::Reentrant))
    );
    println!("clone_value=7 callback_error_retains_mutation=true reentry=Reentrant");
}
