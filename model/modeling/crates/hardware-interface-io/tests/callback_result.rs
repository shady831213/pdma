//! Application callback errors preserve mutations in both storage policies.

use hardware_interface_io::memory::storage::{LocalStorage, MutexStorage, StorageBackend};

fn callback_error_retains_mutation<B: StorageBackend<u64> + Clone>(storage: &B) {
    // Given clone-shared storage.
    let clone = storage.clone();
    // When a callback mutates and then returns its own error.
    let result = storage.mutate(|value| {
        *value = 42;
        Err::<(), _>("application error")
    });
    // Then the outer access succeeds and the clone sees the retained mutation.
    assert_eq!(result, Ok(Err("application error")));
    assert_eq!(clone.inspect(|value| *value), Ok(42));
}

#[test]
fn local_callback_error_does_not_roll_back() {
    callback_error_retains_mutation(&LocalStorage::new(0));
}

#[test]
fn shared_callback_error_does_not_roll_back() {
    callback_error_retains_mutation(&MutexStorage::new(0));
}
