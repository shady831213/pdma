# Reusable library boundaries

The runtime extraction prepares independent libraries for publication while PDMA
remains one integration consumer. Package names retain the provisional
`hardware-interface-*` prefix. No package is published by this change; manifests
retain `publish = false` until names, ownership and release contents are approved.

## Ownership and dependencies

| Package | Responsibility | Normal dependencies |
| --- | --- | --- |
| io | Memory/register interfaces, addresses, atomics, byte backing/allocation, shared storage and observation | std |
| irq | Source/readiness/claim/handler contracts | std; optional Tokio pending-interrupt backend |
| software | SoftwareEnv, main/ISR execution, registry/dispatch, notifications and tracing | irq traits, io, Tokio |
| pipeline | Stepped stages, composition and synchronous/asynchronous drivers | std |
| schema | Layouts, codecs, register definitions and exporters | schema-macros |
| schema-macros | Schema parsing, checking and code generation | proc-macro libraries |

Here `std` means no third-party normal dependencies, not `no_std` support.
IO has no normal package dependency. Its memory module provides both access contracts
and supplied backing/allocation/shared state, without a schema dependency. IRQ defaults to its `tokio` backend;
disabling it retains the source
and handler traits. Software execution requires Tokio, so there is no `async-runtime`
switch. Hardware consumes `io::memory` capabilities; platform assembly selects backing.
Moved APIs have one defining crate, without old register/runtime package aliases.

`SoftwareEnv` is a container for software execution constraints, not an instruction
set model or a replacement async executor. Explicit `EnvLocal` state and
`EnvRegisterAccess` capabilities belong to an environment. Host and Firmware can
use separate environments; ordinary drivers remain independent of that identity.

Access and storage are distinct even for the same bytes. `io::memory` owns the
`MemoryAccess` trait and adapters, plus supplied models with installed regions,
allocation and concrete errors; `io::register` owns register transport/addressing
and observation. Register state and side effects belong to the hardware object, which can
compose schema register windows and provide its own IO endpoint. Shared
arbitrary Rust values use `io::memory::storage::{LocalStorage, MutexStorage, StorageBackend}`.
These are modules within one IO crate, not separate memory/storage packages. Consumers
can implement MemoryAccess using unrelated storage; choosing a built-in model requires
no second interface. `SharedMemoryAccess<S, E>` continues to require
an explicit backend error in named types rather than defaulting to the built-in store.

## Interface generality audit

Memory sideband remains caller-selected and opaque. Backend errors need not be
cloneable or implement `std::error::Error`. Ordinary accesses are byte-order-neutral;
atomic32/64 deliberately operate on naturally aligned little-endian words. That
documented subset is not arbitrary atomic byte-order support. Typed helpers operate
on byte buffers, not Rust object layout or schema codecs. The provided allocator is
a concrete monotonic shared-store allocator; no speculative allocator trait is added.

Independent IRQ delivery requires neither SoftwareEnv nor an ISR registry. The
former ordering requirement on every envelope's vector came from the registry's
BTreeMap; `Ord` now applies only where runtime lookup needs it. A plain source with
a non-ordered notification key can be claimed and passed to a handler. Asynchronous
traits still require Send where documented; a local-only IRQ framework is not added.

Environment affinity applies only to explicit local resources and capabilities.
EnvOwner remains private. Registry internals stay beside dispatch, while
EnvRegisterAccess consumes independent register transport. Splitting packages does
not expose token identity, mutex guards or scheduler internals.

The storage module exposes synchronous scoped closures only. Observation owns its
own synchronization: the blocking gate uses private mutex/reentry bookkeeping and
condition-variable waits; the local gate uses Rc/RefCell ordering state. Both retain
terminal/deferred faults without importing memory. Existing nested access, unwind,
emission-order and waiter-wakeup behavior stays. The local backend's raw access error
is std::cell::BorrowMutError; high-level gate failures retain ObservationGateError.

Register transport supports its documented u32 window offsets and checked u64
address conversion. It does not infer device maps or side effects. The former
memory-backed register-image layer had only examples/tests as consumers and
duplicated schema window validation; it was removed along with its dedicated
tests and schema feature. Current hardware continues to own its register windows.
No speculative address-width or synchronization policy framework is added.

Notifications are coalesced progress hints, distinct from non-consuming IRQ
readiness followed by claim. They and TraceBuffer remain small runtime services;
no additional one-module packages or PDMA protocol abstractions are introduced.

## Verification and release boundary

Tests move to their owning libraries. Mixed environment/register/IRQ tests remain software
integration tests. The repository's native-module-path rule is enforced by PDMA
scenarios, not a distributable macro test scanning a consumer's filesystem.
Independent examples and downstream fixtures exercise custom errors, byte access,
IRQ delivery and composition. PDMA direct and managed scenarios retain behavior.

Local package verification must build the actual archive contents without the PDMA
workspace. Unpublished sibling dependencies may be patched to their extracted
archives for this check; this is not proof of their availability on crates.io.
Before upload, settle final names and owners, verify the declared minimum Rust
version with that toolchain, inspect package files/licenses and publish dependencies
in order. Repository relocation is separate from package naming/version continuity.
Actual PCIe/MMU/RTL features and new PDMA behavior are outside this extraction.

## User-facing API convergence

The first extraction is checkpointed before this API pass. The follow-up keeps
implementation responsibilities while removing ambiguous user entry points:

- Memory data objects are `LocalMemory` and `SharedMemory`; `MemoryPolicy` and its
  local/shared implementations select internal storage. `same_handle` compares
  capability handle identity, not underlying address-space aliasing. Byte-buffer
  helpers are `read_buffer`/`write_buffer`, including explicit sideband variants;
  they still perform no scalar or schema decoding.
- `interrupt_wire` creates an `InterruptEmitter` and `InterruptReceiver`. It is a
  coalescing pending latch with claim, not an electrical level or payload queue.
  Progress notifications remain separate: their waits consume a hint, IRQ readiness
  waits do not consume pending state.
- Register transport and checked address conversion remain in IO. Observation gates,
  callbacks and advanced ordering contracts are grouped under `register::observation`.
  Storage lifetime, environment-bound access and independent observation tests remain;
  fixtures no longer construct a second register-image model to exercise those APIs.
- Trace append accepts an already constructed event. `TraceEntry` owns the sequence
  metadata, and `snapshot` explicitly returns an owned copy. Clearing releases the
  lock before dropping old payloads. Payload cloning during a coherent snapshot
  remains under the lock and must not reenter the trace; no arbitrary callback runs
  during append.
- Runtime onboarding uses scoped `run_main` and in-program ISR registration.
  Static registry/freeze/dispatcher assembly stays documented as an advanced path.
  Heterogeneous handlers retain the explicit typed-enum tradeoff; no erased handler
  architecture or extra scheduler was added. Storage examples explicitly demonstrate
  in-place mutation: callback Err does not imply rollback for `StorageBackend`.

Old synonyms are removed rather than retained as unpublished compatibility APIs.
The concise README recipes and first-use examples supplement the existing adverse
validation examples. The unrelated schema generated-accessor collision for a field
named `value`, noted during ergonomics investigation, remains outside this
runtime-family API pass.
