# Hardware interface software runtime

Concrete Tokio software execution and services: `SoftwareCpu`, explicit CPU-local
resources, interrupt registration and dispatch, coalescing progress hints, and
`TraceBuffer`. This crate requires `std` and Tokio. It is not a hardware capability
umbrella or a configurable scheduler framework. Package names are provisional and
publication is disabled.

Use `hardware-interface-irq` directly for interrupt envelopes, sources, handlers,
and the optional coalescing pending primitive. Use `hardware-interface-register`
for register transport, storage images, and observations, and
`hardware-interface-memory` for memory capabilities. This runtime does not reexport
those APIs. It has no direct memory, storage, schema, or device-protocol dependency.

`SoftwareCpu` treats each real main `Poll::Pending` as a segment boundary while
retaining FIFO position during gate acquisition. `run_handler_turn` holds that gate
for one complete handler future. `CpuLocal<T>` provides clone-shared CPU-local state
with synchronous scoped access that cannot leak references. Ordinary driver state
has no implicit CPU affinity. Explicit register capabilities, CPU-local state, and
interrupt registration retain CPU-specific checks.

Ownership tokens remain implementation details:

```compile_fail
use hardware_interface_runtime::CpuOwner;
```

`InterruptRuntime::run_main` runs a complete software program with a borrowed
`InterruptRegistration` capability. Software registers persistent ISRs directly;
the runtime owns their table and drives the canonical dispatcher. Registration is
CPU-bound, rejects duplicates without replacing handlers, and can occur after the
program suspends. The capability cannot outlive the invocation. Install handlers
before their vectors can arrive; an unregistered vector is a dispatch error.
Frozen-registry entry points remain available for explicit static assembly.
Registration and dispatch require ordered vector keys for the runtime handler table;
this requirement is local to this runtime, not part of the IRQ envelope contract.

Program return requests cooperative dispatch shutdown; an accepted ISR finishes
first. Source/handler failure cancels the waiting program and returns the concrete
error. Dropping the scoped `run_main` future cancels both software futures; it spawns
no background tasks. Interrupt source and handler contracts require Send futures;
this concrete runtime supplies CPU scheduling rather than imposing it on IRQ traits.

`SoftwareCpu::bind_registers(bus)` creates a `CpuRegisterAccess<B>` tied to that CPU,
including when assembly runs outside any CPU context. For a `Sync` register
transport its `hardware_interface_register::RegisterAccess` implementation checks
ownership before constructing or polling every read/write operation. Owner main
and ISR turns can use it; inactive or other CPU polls return `CpuAccessError::Cpu`
before reaching the transport. Transport errors remain `CpuAccessError::Access`.
Cloning a cloneable binding retains its owner. Cancellation retains the underlying
transport's semantics.

`notifications()` creates a `Notifier` and `Notifications`, implementing
`NotificationWait`. This is a coalescing progress hint without interrupt dispatch,
payloads, or handlers. Callers inspect shared state before waiting. One hint is
retained before a waiter arrives, and cancellation preserves an unconsumed hint.
Cloned receivers share one consumable hint; delivery is not broadcast. Sender drop
does not imply EOF; scoped runtime lifetime controls shutdown.

`TraceBuffer` provides shared trace storage without device timing or recording
policy. Register maps, effects, device identity, hardware progress, and allocation
policy remain consumer responsibilities.
