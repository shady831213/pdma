# Hardware interface runtime

Workspace-private mechanics shared by hardware-interface models. It owns generic,
closure-scoped storage contracts and algorithms for checked bulk memory, register
images, addressed register buses, and ordered observations. Concrete local, shared,
and blocking storage or observation policies live in their backend modules. Generic
algorithms name the backend traits and never expose a guard, reference, or borrow
outside the synchronous closure.

The default `async-runtime` feature supplies the Tokio-based software CPU and
interrupt facilities. Hardware consumers can set `default-features = false` to
use the synchronous memory, allocation, storage, register, observation, and trace
APIs without a Tokio dependency. Their public paths and backend behavior remain
the same. This feature separates asynchronous execution dependencies; it does not
make the crate memory-only or `no_std`.

`Memory<P>::add_regions` installs a batch of zero-initialized regions in one synchronous
storage mutation. It checks overlaps and region capacity and reserves all backing and
table storage before changing installed regions. Any returned error preserves existing
regions and bytes; an empty batch is a no-op. Metadata is returned in address order.
`SharedMemory` provides the same operation with shared storage. Region installation
does not expose proposals, revisions, transaction closures, or byte-store snapshots.

`RegisterAccess` remains a compatibility surface. Addressed register
access subtracts a checked `RegisterBusBase` once and then reuses the consumer's
canonical offset decoder. A host can therefore issue full bus addresses while a
device uses generated typed views over the same root image; the runtime does not own a
register map or device effect.

`MemoryAccess::validate` checks complete extents without reading or writing bytes or
allocating backing. It preserves overflow, alignment, lookup, and region-crossing
error order, including mapped zero-length endpoints. `SharedMemoryAccess` exposes
this check together with reads and writes, without storage-management authority.

`MemoryAccess::read_typed<T>` reads into `T::default()` through `AsMut<[u8]>`;
`write_typed` writes through `AsRef<[u8]>`. These default methods use only the byte
backend and preserve its error semantics. Slice length determines access length,
and the caller supplies alignment. They do not encode/decode records or reinterpret
native Rust layouts. Arrays and owned raw-byte types can use them without adding
backend methods. `Self: Sized` keeps the byte trait object-compatible; the existing
`SharedMemoryAccess` wrapper inherits typed access over its erased backend.

`SharedMemoryAllocator` provides a clone-shared monotonic address cursor over a
caller-supplied `SharedMemory`. Nonzero allocations install zeroed backing before
advancing the cursor; failures preserve the cursor and storage. Zero-sized allocations
reserve one logical address without installing backing. The consumer chooses the
starting address and owns device mapping policy and logical zero-mapping semantics.

The crate also contains concrete software CPU, interrupt-dispatch, and trace-buffer
facilities. Those are runtime services rather than schema/storage algorithms. Register
maps and effects, device IDs, epochs, recording collectors, actors, PDMA scheduling,
and timing remain consumer policy.

`SoftwareCpu` treats each real main `Poll::Pending` as a segment boundary while retaining
FIFO position during gate acquisition. `run_handler_turn` holds that gate for one complete
handler future. `CpuOwner` and `CpuLocal<T>` expose opaque validation and synchronous state
access without exposing CPU identity, locks, or await-spanning borrows.

`InterruptRuntime::run_main` runs a complete software program with a borrowed
`InterruptRegistration` capability. Software registers persistent ISRs directly;
the runtime owns their table and drives the same canonical dispatcher used by
`InterruptDispatcher`. Registration is CPU-bound, rejects duplicates without replacing
handlers, and can occur after the program suspends. The capability cannot outlive the
invocation. Install handlers before their vectors can arrive; an unregistered vector
remains a dispatch error. The older frozen-registry entry points remain available for
explicit static runtime assembly, not as a requirement on software programs.

`pending_interrupt(notification)` creates an `InterruptEmitter` and a
`PendingInterrupt<N>` source for one fixed, coalescing destination. Only the emitter
can raise; only the source can wait/claim. Multiple raises before a claim merge;
claim clears atomically, and a subsequent raise remains pending. Source clones
compete for the same pending notification rather than duplicating deliveries.
Waiting is non-consuming and cancellation-safe. A live source keeps the channel
alive even after all emitters are dropped; shutdown is controlled by its runtime.
This component knows no DMA, register mask, cause or software handler, and is not
a queue for per-message payloads. It is provided by the default `async-runtime` feature.

Program return requests cooperative dispatch shutdown; an accepted ISR finishes first.
Source/handler failure cancels the waiting program and returns the concrete error.
Dropping the whole scoped `run_main` future cancels both software futures; it spawns
no background tasks. `CpuOwner::scope` checks ownership before every inner poll of a
capability, including when an already-pending future is repolled from another CPU.

Observation reservations are affine committed state. Consumers reserve only after every
fallible check, inside a non-awaiting commit section, then release effect locks and call
`ObservationReservation::emit` in the same non-cancellable tail. Dropping a reservation,
emitting a mismatched range, synchronization poison, or observer panic permanently faults
the gate and wakes all waiters; missing committed observations are never skipped.

In the four-layer model, `hardware-interface-schema` supplies descriptors and borrowed
views; this crate supplies storage backends and generic memory/register/observation
algorithms; `hardware-interface-pipeline` supplies drive policies; and the consuming
device supplies its effects. None of these runtime facilities creates a second device
execution core.

## Progress notifications

This crate exposes no ring protocol or ring types and has no normal dependency on
PDMA crates. PDMA's shared software ring algorithms live in
[`pdma-model-software`](../software/README.md), using this runtime's generic
transport and CPU capabilities. `MemoryAccess` remains codec-neutral.

Under `async-runtime`, `notifications()` creates a `Notifier` and `Notifications`,
which implements `NotificationWait`. This is a coalescing progress hint without interrupt dispatch,
payloads or handlers. Callers inspect shared state before waiting. One hint is retained
before a waiter arrives, and cancelling a registered waiter preserves its unconsumed
hint. Sender drop does not imply EOF; scoped runtime lifetime controls shutdown.
