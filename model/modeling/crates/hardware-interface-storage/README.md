# hardware-interface-storage

Standard-library-only storage policies for arbitrary values. `LocalStorage<T>`
shares within one thread; `MutexStorage<T>` shares across threads when `T: Send`.
Both implement `StorageBackend<T>` with synchronous `inspect` and `mutate` closures.
References and futures borrowing stored values cannot escape; no raw mutex or lock
guard is exposed.

Calls through clones serialize. Same-thread recursion into the same backend
returns `StorageError::Reentrant` before invoking the nested closure; independent
backends may nest. Mutex poisoning retains and recovers the stored value.
No hardware, register, memory, executor, or device semantics are required.

`tests/storage_backend.rs` exercises arbitrary stored values, cloned access,
reentry, panic recovery, and cross-thread serialization. Compile-fail doctests
check the closure lifetime boundary. Run `cargo test -p hardware-interface-storage`.
