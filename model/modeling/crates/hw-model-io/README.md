# hw-model-io

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
`cargo run -p hw-model-io --example first_memory` for a byte transfer and
`cargo run -p hw-model-io --example callback_result` for shared-state
semantics. Using `MemoryAccess` never requires constructing a supplied model;
`tests/memory_implementations.rs` exercises a separate array implementation.

## Choosing an entry point

| Need | Start here |
| --- | --- |
| Implement access to your own memory | [`memory::MemoryAccess`] |
| Use supplied byte backing | [`memory::LocalMemory`] or [`memory::SharedMemory`] |
| Allocate addresses within a bounded heap | [`memory::SharedMemoryAllocator`] |
| Bind metadata to a memory capability | [`memory::BoundMemory`] |
| Implement asynchronous register transport | [`register::RegisterAccess`] |
| Translate full addresses to register offsets | [`register::AddressedRegisterAccess`] |
| Share arbitrary Rust state through scoped callbacks | [`memory::storage`] |

Start with a byte transfer; the backend allocates only the region you install:

```rust
use hw_model_io::memory::{LocalMemory, MemoryAccess, MemoryAlignment, MemoryRegion};
# fn main() -> Result<(), Box<dyn std::error::Error>> {
let memory = LocalMemory::new();
let address = 0x1000.into();
memory.add_regions(&[MemoryRegion::try_new(address, 4, MemoryAlignment::WORD)?])?;
memory.write_buffer(address, MemoryAlignment::WORD, &42_u32.to_le_bytes())?;
let bytes: [u8; 4] = memory.read_buffer(address, MemoryAlignment::WORD)?;
assert_eq!(u32::from_le_bytes(bytes), 42);
# Ok(())
# }
// Values remain explicitly encoded bytes; no schema or software executor is needed.
```

## Ordered register observations

Observation is optional instrumentation for a register endpoint, not a prerequisite
for implementing `RegisterAccess`. Its APIs live under `register::observation`:

```rust
use hw_model_io::register::observation::{
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
cargo run -p hw-model-io --example advanced_register_observation
cargo test -p hw-model-io --no-default-features
```
