# hw-model-swenv

Concrete Tokio software execution environments and services: `SoftwareEnv`,
explicit environment-local resources, interrupt registration and dispatch, coalescing progress hints, and
`TraceBuffer`. This crate requires `std` and Tokio. It is not a hardware capability
umbrella or a configurable scheduler framework. `swenv` abbreviates software
environment; the public type retains the full name `SoftwareEnv`.

Use `hw-model-irq` directly for interrupt envelopes, sources, handlers,
and the optional coalescing pending primitive. Use `hw-model-io`
for memory and register access capabilities and register observations. Use
its `memory` module for supplied byte backing, allocation and shared-state storage.
This runtime does not reexport those APIs or choose a backing model. It has no
schema or device-protocol dependency.

## Dependency

```toml
[dependencies]
hw-model-swenv = "0.1.0"
hw-model-irq = "0.1.0"
tokio = { version = "1", features = ["macros", "rt"] }
```

Requires Rust 1.96 or later. The dependencies above support the first program
below, including its IRQ source and Tokio entry point. Applications choose any
additional Tokio runtime features they need.

## First program with an ISR

Run `cargo run -p hw-model-swenv --example first_program` for the
commented, executable version. No instruction set, processor registers, OS thread
affinity or cycle timing is modeled: `SoftwareEnv` is a logical execution context
that serializes a main program and its interrupt handlers over Tokio.

Start with scoped software execution. This example registers its ISR in software,
raises a modeled interrupt, and waits for the handler's progress hint:

```rust
use hw_model_irq::{InterruptEnvelope, InterruptHandler, interrupt_wire};
use hw_model_swenv::{InterruptRuntime, SoftwareEnv, Notifier,
    NotificationWait, notifications};
use std::{convert::Infallible, error::Error, sync::Arc};

#[derive(Clone)]
struct Notice;
impl InterruptEnvelope for Notice {
    type Vector = u8;
    fn vector(&self) -> u8 { 1 }
}
struct Handler(Notifier);
impl InterruptHandler<Notice> for Handler {
    type Error = Infallible;
    async fn handle(&mut self, _: &Notice) -> Result<(), Self::Error> {
        self.0.notify();
        Ok(())
    }
}
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn Error>> {
    let (emitter, receiver) = interrupt_wire(Notice);
    let (done, completed) = notifications();
    let runtime = InterruptRuntime::new(Arc::new(SoftwareEnv::new()), receiver);
    let (program, summary) = runtime.run_main(async |isrs| {
        isrs.register(1, Handler(done))?;
        emitter.raise();
        completed.wait_notification().await;
        Ok::<_, hw_model_swenv::InterruptRegistrationError<u8>>(())
    }).await?;
    program?;
    assert_eq!(summary.deliveries(), 1);
    Ok(())
}
```

## Environment and lifetime semantics

`SoftwareEnv` treats each real main `Poll::Pending` as a segment boundary while
retaining FIFO position during gate acquisition. `run_handler_turn` holds that gate
for one complete handler future. `EnvLocal<T>` provides clone-shared environment-local state
with synchronous scoped access that cannot leak references. Ordinary driver state
has no implicit environment affinity. Explicit register capabilities, environment-local state, and
interrupt registration retain environment-specific checks.

An ISR holds its turn across awaits. It must not wait for main or another ISR on
the same context to do work, since those cannot advance until the handler returns.
This is a scheduling guarantee; it does not make external memory writes transactional.

Ownership tokens remain implementation details:

```compile_fail
use hw_model_swenv::EnvOwner;
```

`InterruptRuntime::run_main` runs a complete software program with a borrowed
`InterruptRegistration` capability. Software registers persistent ISRs directly;
the runtime owns their table and drives the canonical dispatcher. Registration is
environment-bound, rejects duplicates without replacing handlers, and can occur after the
program suspends. The capability cannot outlive the invocation. Install handlers
before their vectors can arrive; an unregistered vector is a dispatch error.
Frozen-registry entry points remain available for explicit static assembly.
Registration and dispatch require ordered vector keys for the runtime handler table;
this requirement is local to this runtime, not part of the IRQ envelope contract.

Program return requests cooperative dispatch shutdown; an accepted ISR finishes
first. Source/handler failure cancels the waiting program and returns the concrete
error. Dropping the scoped `run_main` future cancels both software futures; it spawns
no background tasks. Interrupt source and handler contracts require Send futures;
this concrete runtime supplies environment scheduling rather than imposing it on IRQ traits.

`SoftwareEnv::bind_registers(bus)` creates an `EnvRegisterAccess<B>` tied to that environment,
including when assembly runs outside any environment context. For a `Sync` register
transport its `hw_model_io::register::RegisterAccess` implementation checks
ownership before constructing or polling every read/write operation. Owner main
and ISR turns can use it; inactive or other environment polls return `EnvAccessError::Env`
before reaching the transport. Transport errors remain `EnvAccessError::Access`.
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

## Multiple handler types and advanced assembly

`run_main` uses one concrete handler type `H` per invocation. For heterogeneous
handlers, an application enum can delegate to distinct implementations and expose
one error type; this keeps dispatch statically typed:

```rust
use hw_model_irq::{InterruptEnvelope, InterruptHandler};
enum Handlers<A, B> { Control(A), Data(B) }
impl<N, A, B> InterruptHandler<N> for Handlers<A, B>
where
    N: InterruptEnvelope,
    A: InterruptHandler<N>,
    B: InterruptHandler<N, Error = A::Error>,
{
    type Error = A::Error;
    async fn handle(&mut self, notice: &N) -> Result<(), Self::Error> {
        match self {
            Self::Control(handler) => handler.handle(notice).await,
            Self::Data(handler) => handler.handle(notice).await,
        }
    }
}
```

Register `Handlers::Control(control)` and `Handlers::Data(data)` at their respective
vectors. Map differing handler errors into an application enum if needed. The
runtime does not erase or box handler futures. Advanced static assembly can use
`HandlerRegistry::freeze` and `InterruptDispatcher::start`; that API spawns a task.
Request shutdown and await `shutdown_and_join` to observe completion and errors.
Dropping its task handle requests cooperative shutdown but does not join the task;
an accepted handler may still be running.
This differs from dropping a scoped `run_main` future, which immediately cancels
its main and dispatcher futures, and from normal program return, which lets the
accepted handler finish cooperatively.

A `Notifications` wait consumes a coalesced software progress hint. By contrast,
IRQ `InterruptSource::wait_ready` only observes readiness; `claim` consumes the
pending interrupt before the dispatcher waits for its environment turn. Once claimed,
the accepted notification survives cooperative shutdown until its handler finishes;
dropping scoped execution still cancels it. Use progress hints to
recheck shared state, and an IRQ source for dispatch to registered handlers.

Trace insertion accepts a constructed payload: `let sequence = trace.push(event)`.
The buffer assigns metadata atomically with insertion. `snapshot()` returns an
owned `Vec<TraceEntry<E>>` with `sequence` and `event` fields in insertion order;
cloned buffers share events and sequence allocation. Sequences saturate at
`usize::MAX`. `clear()` resets allocation to zero and drops removed payloads after
unlocking, so a destructor may append a new event. Snapshot clones payloads while
holding the lock to preserve a coherent view: payload `Clone` must not reenter the
same trace or wait for another operation on it. Use simple owned payloads or `Arc`
payloads when cloning must be cheap. Snapshots remain valid after clear.
