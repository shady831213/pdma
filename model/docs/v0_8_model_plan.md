# PDMA v0.8 Model Plan

## Status

Completed as a workspace slice on 2026-07-16. The pre-v0.8 baseline is
commit `0c044bd`; this document describes the current implementation and does not claim
a v0.8 commit hash.

## Purpose

v0.8 extends the completed v0.7 host-owned IRQ model. It keeps the device-to-host
notification boundary, but replaces the historical single software handler with
deterministic host dispatch by `IrqVector`. It also makes mutable software resources
explicitly single-copy across any number of main/handler switches.

The modeled path remains:

```text
PDMA source -> Line or Message notification -> host vector lookup
            -> one persistent vector handler -> driver source demultiplex/ack
```

Source routing is many-to-one: different completion/event sources may select the same
vector. Handler routing is one entry per vector. Message routes are MSI/MSI-X-like at
the vector-dispatch level, but concrete PCIe address/data, table/PBA, and transport
details remain deferred.

## Preserved Boundaries

| Crate | v0.8 responsibility |
| --- | --- |
| `pdma-model-contract` | Runtime-free IRQ vectors/notifications and typed duplicate, unregistered, and CPU-local invariant errors. |
| `pdma-model-device` | PDMA-local source enable/mask/pending/ack plus Line assertion and discrete Message generation. No handler storage or invocation. |
| `pdma-model-host` | Modeled software CPU, CPU-local state, shared driver accounting, source demultiplexing, and vector-to-handler delivery. |
| `pdma-model-scenarios` | Explicit device worker, one complete main future, delivery future, bootstrap synchronization, trace, and final assertions. |

The one-device system still has no duplicate host enable/mask/completion layer. Device
mask/ack remains distinct from host vector lookup and software handler dispatch.

## Notification and Registry Contract

- `IrqNotification::vector()` resolves the configured vector for both stateful Line
  snapshots and discrete Message notifications.
- `IrqDelivery<P, NoHandler>` cannot run. First registration consumes `NoHandler`,
  creates `HasHandlers`, and returns the control handle.
- Additional registrations are allowed only before `run`. The registry is a
  deterministic `BTreeMap<IrqVector, Box<dyn IrqHandler>>`.
- Duplicate registration returns `ModelError::DuplicateIrqVector` without replacing the
  original persistent handler.
- Delivery to an unknown vector returns `ModelError::UnregisteredIrqVector` before CPU
  acquisition, handler trace emission, or successful-delivery accounting.
- Handler objects may borrow caller-owned state. Different vectors may use heterogeneous
  handler types.
- One delivery actor invokes one selected handler at a time. Handlers remain atomic,
  serial, and non-nested across vectors.

Line and Message notifications share the same host lookup. Message identity remains
source-scoped and monotonic, and every representable trigger remains a separate
delivery. The model performs no intentional coalescing or moderation. Redundant
messages are handled by idempotent driver drain/ack behavior.

## Software CPU and CPU-Local State

`SoftwareCpu::run_main` still accepts one complete main future. Each poll is one atomic
main segment; only a real `Poll::Pending` releases the FIFO CPU gate. An immediately
ready `.await` remains in the same segment.

`SoftwareCpu::run_irq` holds that same gate for the complete handler future, including
its suspended intervals. The private active CPU identity is installed only while the
main or handler future is being polled. Therefore:

- main and handler code never execute concurrently;
- handlers cannot nest;
- an off-context access observes `ModelError::SoftwareCpuInactive`;
- access from another modeled CPU observes `ModelError::SoftwareCpuMismatch`;
- reentrant access observes `ModelError::SoftwareCpuStateBusy`.

`CpuLocal<T>` binds one `T` to one private `SoftwareCpu` identity. Cloning a
`CpuLocal<T>` clones only the capability and shares the same allocation, even when
`T: !Clone`. `CpuLocal::with` uses synchronous `try_lock` closure access. It exposes no
identity, mutex, guard, `Deref`, reference escape, or async borrow that could cross an
`.await`. Cancellation removes the active identity and releases the CPU gate through
RAII, so later main or handler polls can reacquire both safely.

FIFO ordering begins only after an actor requests the CPU gate. An IRQ notification that
host delivery has not yet observed has no priority, and a later IRQ request cannot
overtake an already queued main request. v0.8 adds no broader scheduler-priority
contract.

## Shared Driver Accounting and Cancellation

Software-bearing setup uses `AsyncDriver::setup_on` or
`setup_with_config_on`. It captures the active private CPU identity before calling
`create_queue` and validates that owner before every nested poll of the create future.
Off-context setup, or a pending setup resumed inactive/on another CPU, therefore returns
the typed CPU error before queue creation or queue-ID consumption.

When one of the five zero-entry system-scenario observability tests needs a driver, it uses
the explicit `setup_manual_on`/`setup_manual_with_config_on` path. There are exactly four
scenario call sites; a focused host integration test separately verifies manual-mode parity:
one in the step-budget test, one in the manual run-until-blocked test, and two for the
baseline/traced drivers in the manual tracing comparison. Manual setup is not an
IRQ-sharing escape hatch.

An `AsyncDriver` stores immutable queue identity separately from one shared accounting
allocation. Every clone observes the same next-command ID, free-slot budget, and
deferred rollback queue. A common adapter validates the owning active `SoftwareCpu`
before every inner poll of CPU-bound create, submit, doorbell, completion, and event
port futures. A future that first returned `Pending` on its owner cannot later commit
when resumed outside software CPU execution or on another CPU; it returns
`SoftwareCpuInactive` or `SoftwareCpuMismatch` before polling the inner port operation.
Manual accounting uses the same adapter without adding a modeled CPU identity.

Submission and polling semantics are:

1. Queue creation and queue-ID allocation commit only when `create_queue` returns
   `Ok(_)`; dropping it while pending leaves both unchanged.
2. Prepare drains prior deferred rollbacks, consumes the next command ID, and reserves
   one local slot.
3. Submit failure, invalid-context migration, or cancellation before device acceptance
   drops an armed reservation.
   Drop queues one rollback without accessing CPU-local state. The next accounting
   operation restores the slot, while the command ID remains consumed.
4. Successful `submit_command` is device acceptance. The reservation is synchronously
   disarmed in the same poll before the doorbell future is polled.
5. Runnable/doorbell state commits only when `ring_doorbell` returns `Ok(())`. Doorbell
   failure, invalid-context migration, pending-drop cancellation, or cancellation after
   acceptance keeps the command ID and slot consumed because the device already owns
   the command, while an uncommitted doorbell leaves runnable state unchanged.
6. A completion/event record is removed only when the port returns `Ok(Some(_))`; the
   driver then reclaims its slot synchronously in that same poll, before another
   suspension is possible.

No CPU-local state borrow spans a device-port `.await`. Completion and event reclaim,
queue-full behavior, submit failure, inner-future cancellation, whole-main abort,
doorbell failure, and doorbell cancellation are covered by deterministic host tests.
The `AsyncDevicePort` commit-point contract requires queue creation only on `Ok(_)`,
command acceptance and doorbell/runnable mutation only on `Ok(())`, and
completion/event removal only on `Ok(Some(_))`. Dropping any of those futures while
`Pending` leaves the queue ID/state, command acceptance, runnable state, or record
unchanged.

## Scenario Bootstrap and Alternation

IRQ scenarios keep three explicit concurrent actors: device worker, one complete
software main, and host delivery.

Main performs CPU-bound driver setup, retains its driver, and publishes one clone plus
the completion/event sources over a one-shot bootstrap channel. Delivery receives the
clone outside the modeled CPU, constructs every vector handler, completes registration,
and signals readiness. Main waits for readiness before configuring routes,
enabling/unmasking sources, or submitting work. No handler waits for initialization
while holding the CPU.

The four IRQ scenarios prove:

- Line completion lifecycle through the host registry and device ack/deassertion;
- completion and event sources sharing one persistent Message-vector handler;
- two discrete redundant Message deliveries with no coalescing;
- distinct vectors selecting distinct handlers through repeated
  `main -> handler A -> main -> handler B -> main` rounds.

The distinct-vector story uses command depth one. Handler A/B clones reclaim accounting
credit, main's retained clone submits the next operation, and a final retained-main
submission succeeds. Milestones prove a pending handler blocks main and the other vector
until it returns; trace and summary counts prove all successful turns.

## Validation Invariant

The scenario suite contains 54 tests:

- 49 software-bearing tests, each with exactly one complete `run_main`;
- five zero-entry pure device/manual observability tests:
  - `reports_step_budget_exhaustion_when_work_remains`;
  - `async_runner_no_work_reports_idle_status`;
  - `async_manual_run_until_blocked_returns_terminal_status_without_losing_idle_errors`;
  - `async_tracing_preserves_manual_run_until_idle_behavior`;
  - `async_trace_log_records_idle_status_steps`.

The IRQ scenario binary contains exactly four tests and four `run_main` calls. The five
zero-entry tests contain exactly four explicit manual setup calls.

Workspace closeout gates are:

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps --document-private-items
```

## Non-Goals

v0.8 adds no device-owned handlers, device-to-host dependency, duplicate system
enable/mask layer, handler nesting or parallelism, priority scheduler, intentional
coalescing, cycle timing, descriptor/register ABI, generated headers, unsafe code,
external dependency, concrete PCIe MSI/MSI-X transport, MSI-X table/PBA, or APIC/GIC
integration.
