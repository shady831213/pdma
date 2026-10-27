# hardware-interface-io

Memory and register access capabilities using only the standard library, with no
normal package dependencies. `memory` exposes byte addresses, checked alignments,
opaque sideband, backend-owned errors, atomic vocabulary and supplied memory models.
`register` exposes
asynchronous offset access, full-address translation and ordered observations.

Register access differs from ordinary byte transfers: registers may require exact
widths, enforce permissions and produce side effects. Implement either capability
against your own backend, or use `memory::{LocalMemory, SharedMemory}` for byte storage.
`memory::storage` provides closure-scoped local/shared state for backing policies.
A hardware model can compose schema register windows and implement `RegisterAccess`
at its transport boundary, applying the model's own register side effects there.

`memory::MemoryAccess<S>` preserves the backend's error type. `BoundMemory` fixes
one sideband value; `SharedMemoryAccess<S, E>` erases a thread-safe backend while
requiring an explicit error type. Atomic32/64 operations use naturally aligned
little-endian words; unsupported backends return `AtomicAccessError::Unsupported`.
Byte-buffer helpers do not encode or decode Rust scalar values.

The memory module documents both interface and backing use. Run
`cargo run -p hardware-interface-io --example first_memory` for a byte transfer and
`cargo run -p hardware-interface-io --example callback_result` for shared-state
semantics. Using `MemoryAccess` never requires constructing a supplied model;
`tests/memory_implementations.rs` exercises a separate array implementation.

Observation APIs live together under `register::observation`:

```rust
use hardware_interface_io::register::observation::{
    AccessOrigin, NoopRegisterObserver, ObservationGate, RegisterObservation,
    SharedRegisterObserver,
};
use std::sync::Arc;
# fn main() -> Result<(), Box<dyn std::error::Error>> {
let gate = ObservationGate::new(SharedRegisterObserver::new(Arc::new(NoopRegisterObserver)));
let reservation = gate.reserve(1)?;
let observation = RegisterObservation::write(
    reservation.start_sequence(), AccessOrigin::Direct, 0, &[42, 0, 0, 0],
);
reservation.emit(vec![observation])?;
# Ok(())
# }
```

`ObservationGate` is clone-shared and blocking; `LocalObservationGate` provides
local delivery. Advanced backend implementations use `ObservationBackend`,
`ObservationOrder` and affine receipts in the same module. State-closure reentry
faults before locking. Observer callbacks run outside the state lock and may
inspect or reserve later work; recursive emission faults. Poisoning, abandoned
reservations and callback panics are terminal, and waiting emitters wake on fault.
The local gate owns its ordering state in `Rc<RefCell<_>>`; direct backend access
reports `std::cell::BorrowMutError` on reentry. It does not require a memory backing
or a shared-storage library.

Run the advanced example for commit, rejection and cancellation:

```sh
cargo run -p hardware-interface-io --example advanced_register_observation
cargo test -p hardware-interface-io --no-default-features
```
