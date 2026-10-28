# Shared state storage

Standard-library-only storage policies for arbitrary values. `LocalStorage<T>`
shares within one thread; `MutexStorage<T>` shares across threads when `T: Send`.
Both implement `StorageBackend<T>` with synchronous `inspect` and `mutate` closures.
References and futures borrowing stored values cannot escape; no raw mutex or lock
guard is exposed.

`LocalStorage` is neither `Send` nor `Sync`; `MutexStorage` is both when `T: Send`.
Closures cannot suspend and their returned values cannot borrow the stored value.
Choose this layer when you need shared state without any memory-address or register
semantics; those capabilities are provided by the memory model and IO interfaces.

Calls through clones serialize. Same-thread recursion into the same backend
returns `StorageError::Reentrant` before invoking the nested closure; independent
backends may nest. Mutex poisoning retains and recovers the stored value.
No hardware, register, memory, executor, or device semantics are required.

`tests/storage_backend.rs` exercises arbitrary stored values, cloned access,
reentry, panic recovery, and cross-thread serialization. Compile-fail doctests
check the closure lifetime boundary. Run `cargo test -p hw-model-io`.

Cloning a backend shares its value; it does not copy the value. Callback results
are returned inside the access result: an application `Err` does **not** roll back
mutations. Neither backend implements transactions. Validate before mutation or
prepare a replacement separately when application errors must leave state intact.

```rust
use hw_model_io::memory::storage::{LocalStorage, StorageBackend, StorageError};
let storage = LocalStorage::new(0);
let clone = storage.clone();
let result = storage.mutate(|value| {
    *value = 7;
    Err::<(), _>("application failure")
});
assert_eq!(result, Ok(Err("application failure")));
assert_eq!(clone.inspect(|value| *value), Ok(7));
assert_eq!(storage.inspect(|_| clone.inspect(|value| *value)),
           Ok(Err(StorageError::Reentrant)));
```

The same callback-result and reentry policies apply to `MutexStorage`.

Run `cargo run -p hw-model-io --example callback_result` for the
clone-sharing, application-error, and reentry example.
