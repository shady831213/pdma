# PDMA v0 Architecture Specification

## Purpose and Scope

This document records the validated behavioral contract through the COMPLETE v0.9
historical data ABI and exact 4 KiB numeric MMIO memory-and-bus contract. White-board
architecture documents are background only. This spec defers anything not exercised by
the current scenario tests, including cycle timing, callable C APIs, concrete PCIe/MSI-X
transport, and real operating-system integration.

The intended readers are implementers who will extend the model, port it to a cycle-accurate or RTL context, or write future drivers against a more concrete ABI.

## Validated v0 Model Contract

The v0 model validates the following end-to-end behaviors:

- Typed object identifiers for queues, commands, buffers, and completion rings.
- Checked device memory with explicit bounds and range-overflow errors.
- Bounded command, completion, and event rings with full/empty/wrap behavior.
- Driver queue setup, submission, doorbell, completion polling, and event polling.
- Device step loop that advances at most one command per `step()` and reports budget exhaustion in `run_until_idle()`.
- Native operation dispatch and resource-demand derivation from external commands.
- Linear copy, 2D strided copy, broadcast copy, and byte-wise wrapping vector add.
- Queue directory and explicit per-queue configuration objects.
- Deterministic multi-queue arbitration with round-robin starvation avoidance.
- Recoverable memory-fault events that skip success completions.
- Resource-credit gating and observable stall annotations.
- Deterministic trace sink with structured in-memory events and Chrome/Perfetto JSON export.
- PDMA-local completion/event IRQ sources with Line and Message routes.
- Host-owned IRQ delivery through a fixed vector-to-handler registry and a CPU token
  shared with SW main.
- One complete software-main future whose real async suspension points define interruptible
  execution boundaries.
- CPU-bound, closure-scoped single-copy software state shared across repeated
  main/handler switches.
- Shared driver accounting with explicit cancellation semantics before and after device
  command acceptance.
- ABI-v1 fixed-width little-endian descriptor, completion, event, DMA address, ring, and typed
  control vocabulary, with a checked-in C11 data header generated from Rust metadata.
- One system-owned `SharedMemory` per device instance, with PDMA mapping policy retained
  by the device and checked bulk access supplied by `hardware-interface-runtime`.
- Whole-device reset with checked epoch advancement, stale-capability rejection before
  mutation, and fresh bootstrap under the new epoch.
- One exact 4 KiB little-endian MMIO region with 66 canonical registers, shared decoding,
  generated C constants, command banks, and stable response semantics.
- One canonical `PdmaRegisterPort<AsyncDeviceAccess>` whose split memory/register,
  provisioning, closed-batch, and compound-claim capabilities reuse existing typed
  boundaries.

The contract is deterministic: the same initial state, resource configuration, arbitration policy, and command sequence always produces the same memory contents, completion records, event records, and trace event order.

## Exact MMIO and Ordered Commit Contract

Offsets `0x000..=0xfff` contain exact 4/8-byte little-endian registers. Wrong widths,
misalignment, cross-register extents, reserved offsets/bits, and permission faults return
the closed transport error before mutation. The 66-entry registry is the only
offset/access/reset/mask authority and drives Rust decoding and C11 constants. Global,
queue, and IRQ families have 4, 9, and 9 commands and share 32 result codes.

Standalone activation validates three exact mappings already committed in device-owned mapping state
and uses the prepared mapped-activation seam; it commits no DMA. The closed bootstrap
batch combines three staged rings with activation, while the closed submission batch
combines optional indirect DMA and descriptor bytes with producer publication. Both
validate every fallible stage before one synchronous model-state commit. This is a
cancel-safe modeling mechanism, not a hardware transaction or cycle-accurate claim. Doorbell capture is a
later command, preserving bytes-before-producer-before-runnable ordering.

`PdmaRegisterPort` owns clone-shared selector state. Notification readiness is
non-consuming; `PdmaInterruptSource` performs one compound exact claim after wake.
The canonical scenario injects one system-owned register observer through `DeviceCore`.
It proves 156 committed observations: 154 Direct entries and the two real Batch-origin
queue-command writes, while a scenario-local memory decorator records bulk access
separately. Worker and dispatcher shutdown/join finish before the collector snapshot.

Schema definitions and resolved layouts belong to `hardware-interface-schema`; shared
memory, raw registers, CPU-local state, generic interrupt dispatch, and trace storage
belong to `hardware-interface-runtime`. Contract `semantic`, `abi`, and `bridge` modules
own conceptual vocabulary, exact bytes/registers, and cross-layer conversion
respectively. Device owns mappings and execution/retirement. Host owns the canonical
register port, PDMA interrupt adapter, driver accounting, and explicit semantic/sync
compatibility adapters. Compatibility does not create a second executor or canonical
E2E.

## Software/Driver Responsibility Boundary

The driver model, implemented in `model/modeling/crates/host/src/driver.rs`, owns the following responsibilities:

- Allocate device-visible buffers through the active `DriverPort` implementation and
  fill source data through `write_buffer_on`. The canonical path is
  `PdmaRegisterPort<AsyncDeviceAccess>`; `compat::AsyncDevicePort` and
  `compat::SemanticDevicePort` are explicit compatibility boundaries only.
- Configure a CPU-bound queue with `AsyncDriver::setup_on` or
  `AsyncDriver::setup_with_config_on`, receiving a `QueueConfigHandle`. The active
  software CPU is captured before device queue creation and revalidated before every
  nested create poll. `AsyncDriver` exclusively services IRQs.
- Use CPU-unbound `ManualDriver` only as the explicit observability facade, constructed
  with `ManualDriver::setup_on` or `ManualDriver::setup_with_config_on`; it has no IRQ
  service. Among system scenarios, only the five named no-software observability tests
  use it. A focused host integration test also exercises manual-mode parity without
  representing a software-bearing scenario.
- Assign monotonic per-queue command IDs and track free local command slots in one
  private shared core with one accounting allocation shared by all clones of either
  driver facade.
- Build external request structs (`CopyRequest`, `StridedCopyRequest`, `BroadcastCopyRequest`, `VectorAddRequest`) and call the matching `submit_*` helper.
- Ring the implicit doorbell by submitting through the driver helper.
- Validate the owning active software CPU before every nested create, submit, doorbell,
  completion, and event port poll. A pending future resumed inactive or on another CPU
  returns the typed error before the inner operation's commit point.
- Reclaim one local slot synchronously for each successfully returned completion/event
  record.
- Configure device IRQ routes and source controls through the host device port.
- Demultiplex IRQ sources, drain the relevant completion/event path, then acknowledge the device cause.

The driver does not access the command ring directly, does not execute commands, and
does not rely on wall-clock timing or cycle counts. A CPU-bound driver rejects
off-context or wrong-CPU access, including migration between polls of one port future,
instead of silently falling back to manual state.

Each software-bearing system scenario enters this boundary exactly once through
`SoftwareCpu::run_main`. Driver setup, device IRQ configuration, buffer access,
submission, and software polling or waiting form one complete main future rather than a
sequence of manually scheduled turns.

## Device Responsibility Boundary

The device model, implemented under `model/modeling/crates/device/src/runner/`, owns the following responsibilities:

- Maintain device memory, the queue directory, command rings, completion rings, queue-tagged events, the runnable queue list, resource configuration, IRQ state, and the trace sink.
- Accept submissions through `submit_command`, emit a trace event, and queue the command on the correct command ring.
- Capture doorbells through `ring_doorbell` and mark queues runnable.
- Select the next queue through deterministic runnable-queue ordering.
- Dispatch the selected external command to its operation-specific native execution path.
- Check execution resource demand, then run copy, broadcast, or vector-add behavior.
- Publish success completions to the bound completion ring, or publish memory-fault events to the event ring.
- Stall deterministically when resources or publication slots are exhausted, and retry pending retirements on later steps.
- Expose tracing controls and exported JSON without changing behavior when tracing is disabled.
- Latch IRQ causes only after completion/event publication, assert/deassert Line routes,
  and preserve every discrete Message trigger.

The device never stores or invokes software handlers. It does not expose wall-clock or
cycle timing. Its semantic and ABI queue frontends converge on one execution and
retirement core: both use the same deterministic scheduling, resource, operation, result
publication, and PDMA-local IRQ behavior rather than a second executor.

### Whole-Device Reset and Epoch Semantics

`WholeDeviceReset` succeeds only when its expected epoch matches both device and bus
control epochs, then checked-advances both together. Success preserves committed
`DmaSpace` regions, bytes, and allocator generation, trace history, resource
configuration, and lifecycle accounting. It clears device queues, completion/event
records, runnable work, pending retirement, bus-control queue/cursor state, and
PDMA-local IRQ routes, causes, and queued notifications. A fresh bootstrap may reuse
numeric queue and source IDs, but only with the new `DeviceEpoch`.

Every queue, IRQ source, and notification capability is epoch-bearing. A stale
capability, stale reset request, or stale notification acknowledgement fails before
mutation; an epoch-overflow reset also leaves state unchanged. Reset therefore cannot
publish discarded stalled or pending-retirement work later.

## Queue Directory, Queue Configuration, Command Ring, Completion Ring, and Event Ring Ownership

Queue directory entries (`QueueDirectoryEntry` in `model/modeling/crates/contract/src/queue.rs`) bind together:

- A `QueueId` assigned by the device.
- A bound `CompletionRingId`.
- A `QueueConfig` supplied by the driver, containing command ring depth, completion ring depth, event routing policy, and a software owner ID.

The device owns the storage for all command rings, completion rings, and queue-tagged events. The driver owns the returned `QueueConfigHandle`, which is the only driver-visible token for that queue and completion ring. Local driver slot accounting is a driver-side shadow; the device never tracks how many commands the driver thinks are outstanding.

For event routing, v0 supports only `EventRouting::QueueLocal`. A driver may poll only events whose `queue_id` matches its own queue through `poll_queue_event`.

## Submission, Doorbell/Runnable, Scheduling, Execution, Completion, and Event Flows

### Submission

CPU-bound queue creation, submission, doorbell, completion, and event futures validate
the owning CPU before every nested port poll. If one first returns `Pending` on its
owner and is later resumed inactive or on another CPU, the typed CPU error is returned
before the inner port operation can reach its commit point. Manual accounting uses the
same polling adapter without adding CPU identity restrictions.

The driver builds a request, wraps it in an external command descriptor, and calls
`submit_*`. Preparation drains deferred rollbacks, consumes the next command ID, and
reserves one local slot. If submission fails, migrates to an invalid CPU context, or is
cancelled before `submit_command` accepts the command, reservation Drop queues a
deferred slot rollback; the command ID remains consumed. The next accounting access
drains the rollback.

Successful `submit_command` is the device-acceptance point. The reservation is disarmed
synchronously in that same poll before the doorbell future is polled, so doorbell
failure, invalid-context migration, or cancellation keeps both the accepted command and
its local slot consumed while an uncommitted doorbell leaves runnable state unchanged.
The port removes a completion/event record only on `Ok(Some(_))`, after which the driver
reclaims its slot synchronously in the same poll before another suspension is possible.
An invalid-context attempt cannot consume the record or credit. No accounting borrow
spans a device-port poll or await. A `TraceEventKind::CommandSubmitted` event is emitted
when the device accepts the command.

`DriverPort` defines these cancel-safe commit points: `create_queue` creates the
queue and consumes its ID only on `Ok(_)`; `submit_command` accepts the command only on
`Ok(())`; `ring_doorbell` mutates runnable state and trace only on `Ok(())`; and
completion/event polling removes a record only on `Ok(Some(_))`. Dropping any of those
futures while `Pending` leaves the corresponding queue, ID, command, runnable state,
trace, or record unchanged.

### Doorbell/Runnable

`ring_doorbell` adds the queue to the runnable list if it is not already present. A `TraceEventKind::DoorbellRunnable` event is emitted.

### Scheduling

One native step pops the next runnable queue. If none exists, the device records `TraceEventKind::Idle` and reports `AsyncStepStatus::Idle`. If a queue is selected, the command at the front of its command ring is cloned and a `TraceEventKind::Scheduled` event is emitted before operation dispatch.

### Execution

The device computes resource demand for the command. If resources are insufficient, the queue is re-queued as runnable, a `TraceEventKind::Stall` event is emitted, and `AsyncStepStatus::Stalled(ExecutionResourcePressure)` is reported. Otherwise the operation-specific native path runs.

On success, `TraceEventKind::ExecutionSucceeded` is emitted. On a memory fault, `TraceEventKind::ExecutionFaulted` is emitted and the fault is routed to the event path.

### Completion

A successful command produces one `CompletionRecord` with `CompletionStatus::Success`. If the completion ring is full or publication credits are exhausted, the retirement becomes pending, a stall event is emitted, and the device reports `AsyncStepStatus::Stalled(CompletionPublicationPressure)`. When publication succeeds, `TraceEventKind::CompletionPublished` is emitted. The driver later calls `poll_completion`, which emits `TraceEventKind::CompletionPolled`.

### Event

A memory fault produces one `EventRecord` with an `EventErrorKind`. If event publication credits are exhausted, the retirement becomes pending, a stall event is emitted, and the device reports `AsyncStepStatus::Stalled(EventPublicationPressure)` or `FaultRetirementPendingRetry`. When publication succeeds, `TraceEventKind::EventPublished` is emitted and the original `ModelError` is observed by the worker. The driver polls its queue-local event path, emitting `TraceEventKind::EventPolled`.

## IRQ Delivery Semantics

Completion and queue-local event publication each identify a PDMA-local `IrqSource`.
Source enable, mask, pending cause, route, and ack state belong to the device. Masking
preserves pending work. Acknowledging clears a selected Line cause only when its
completion/event path is empty; a record published after software's final empty poll
keeps the cause pending.

`IrqRoute::Line` models stateful delivery. An enabled, unmasked pending source asserts
its line, and device ack or masking recomputes/deasserts it. `IrqRoute::Message` models a
discrete delivery transaction carrying an `IrqVector`, source, and source-scoped
monotonic `IrqMessageId`. Each trigger within representable model state is retained
separately; the model performs no intentional coalescing or moderation. Exact PCIe
MSI/MSI-X address/data, table/PBA, and transport behavior remain deferred.

Both Line and Message notifications resolve one `IrqVector` at the shared contract
boundary. Runtime `HandlerRegistry` freezes a non-empty vector-to-handler map before
`InterruptDispatcher` starts and never places a handler in the device. Host
`PdmaIrqDelivery` supplies the PDMA source, service, trace, and error adapters around
that generic dispatcher.
Different sources may share one vector and persistent handler. Duplicate registration
returns `DuplicateIrqVector` without replacement. An unregistered vector returns
`UnregisteredIrqVector` before CPU acquisition, lifecycle tracing, or delivery-summary
increment.

`SoftwareCpu::run_main` receives one complete main future. One main atomic segment is
exactly one poll of that future, ending in `Poll::Ready` or `Poll::Pending`; only a real
`Poll::Pending` releases the CPU for later main reacquisition. Consequently, an
`.await` whose awaited future is immediately ready does not create an interrupt
boundary.

The complete handler future owns the same CPU token across all of its polls and
suspensions. Main and IRQ contexts therefore do not overlap, handlers remain atomic,
serial, and non-nested across vectors, and the independent device worker may still
progress. CPU ordering follows the Tokio mutex's FIFO request order: an IRQ precedes
main reacquisition only after the delivery actor has observed a notification and
requested the gate first. An unobserved device notification has no CPU priority, and a
later IRQ request does not overtake an already queued main request; the model makes no
broader scheduler-priority promise.

`CpuLocal<T>` binds one mutable value to one private `SoftwareCpu` identity. Cloned
capabilities share that single value without requiring `T: Clone`. The private
`SoftwareCpuToken` is installed in the single `CURRENT_SOFTWARE_CPU` option only around
an actual inner main or handler poll; its RAII guard restores any prior token afterward.
Synchronous `CpuLocal::with` access returns typed inactive, mismatch, or busy errors and
uses `try_lock`; it exposes no identity, lock, guard, reference escape, or async borrow
across an await. Cancellation removes the active identity and releases the CPU gate
through RAII.

Driver IRQ service reads the delivered source, drains completion/event records, then
acknowledges the device cause. This ordering is idempotent for redundant message
deliveries, and the device retains a Line cause if a new record appears before ack.
Shutdown has deterministic priority over a simultaneously ready notification. Delivery
progress is synchronized by handler-owned channels when a scenario needs it, while the
actor's final `IrqDeliverySummary` reports only successfully completed handler turns; no
separate delivery-progress watch API exists. Evidence:
`crates/device/tests/irq_line_ack.rs`, `crates/host/tests/irq_runtime_surface.rs`, and
`crates/scenarios/tests/irq_delivery.rs`.

IRQ scenarios bootstrap without transferring driver ownership away from main. Main
performs CPU-bound setup, retains one driver capability, and publishes a clone plus
source identities over a one-shot channel. Delivery receives that clone outside the
modeled CPU, constructs and registers the vector handlers, then signals readiness. Main
waits for readiness before configuring routes, enabling/unmasking sources, or
submitting. No handler waits for initialization while holding the CPU. The depth-one
alternation story proves repeated `main -> handler A -> main -> handler B -> main`
switches and a later submission through main's retained capability.

## Historical Completed Descriptor-Layout ABI Baseline

The following ABI-v1 descriptor, completion, and event record remains the completed
historical baseline alongside the completed exact MMIO contract.

## ABI-v1 and Command Boundary

The historical v0.9 ABI established raw DMA allocation/read/write, typed control
transactions, epoch-bearing IRQ notification waiting, atomic queue bootstrap, and
atomic submission before the separate doorbell commit. Its retired transport adapters
are preserved by name only in `v0_9_model_plan.md`. The active successor expresses the
same behavior through `PdmaRegisterPort<AsyncDeviceAccess>` over runtime
`MemoryAccess`/`RegisterAccess`, device-backed `PdmaMemoryProvisioning`, closed
`PdmaBatchAccess`, and `PdmaExactInterruptClaim`. The port validates its captured
`SoftwareCpu` before every nested CPU-bound poll and never retains a host-state guard
across an await. `PdmaMappedRegion` carries the host object-to-`MemoryAddress` handoff;
committed regions remain mapped for the device lifetime. Explicit compatibility ports
exercise the same core and do not create a second E2E path.

The ABI is fixed-width little-endian data, never Rust layout: v1 descriptors are 64
bytes, completions 32 bytes, and events 64 bytes. `include/pdma_model_abi.h` is a
checked-in generated C11 data header, not a callable C API. It is rendered from Rust
metadata with `cargo run -p pdma-model-contract --bin pdma-abi-header -- --write
include/pdma_model_abi.h`; gates must run the corresponding `--check` command and reject
any byte mismatch.

`DmaAddress` is the opaque device-visible address carried by descriptors and rings. It
does not identify whether a platform uses physical, bus, or translated addressing.

## Conceptual Command Boundary

External commands live in `model/modeling/crates/contract/src/desc.rs` and represent the
conceptual driver-visible descriptor surface. The `ExternalCommand` enum carries `Copy`,
`StridedCopy`, `BroadcastCopy`, and `VectorAdd` variants, each with queue ID, completion
ring ID, command ID, and request payload. The semantic frontend lowers that public form
once into a device-private `ExecutionCommand`; the exact frontend decodes ABI-v1 and
resolves its DMA addresses directly into the same private representation. The shared
native core matches only private execution operations and derives operation-specific
chunks and resource demand internally. The selected queue context supplies execution
queue identity, while the semantic command or exact queue directory supplies completion
routing. ABI-v1 encodes equivalent command data through the fixed registry; neither the
public command type nor the private execution type is wire layout.

## Supported v0 Operations

### Linear Copy

`CopyRequest` in `desc.rs` takes a source buffer, destination buffer, and byte length. The native copy path reads and writes that single range directly. Evidence: `tests/linear_copy.rs`.

### Strided Copy

`StridedCopyRequest` takes source and destination buffers, row count, row bytes, and source/destination strides. The native copy path derives one `NativeCopyChunk` per row. Stride multiplication is checked for overflow and maps to `RangeOverflow`. Evidence: `tests/strided_copy.rs`.

### Broadcast Copy

`BroadcastCopyRequest` takes a source buffer with offset and length plus a list of destination ranges. The native broadcast path iterates the destinations and rereads the source for each one. Execution stops on the first memory fault and publishes a single event; otherwise a single success completion is published. Evidence: `tests/broadcast_copy.rs`.

### Vector Add

`VectorAddRequest` takes two source buffer ranges and one destination range plus a length. The vector-add engine reads both source ranges, performs byte-wise wrapping addition, and writes the result. Evidence: `tests/vector_add.rs`.

## Completion, Event, Memory-Fault, Resource-Credit, and Stall Semantics

### Completion Semantics

A success completion is one `CompletionRecord` per successful command. It carries the queue ID, command ID, and `CompletionStatus::Success`. Completions are published to the completion ring bound at queue creation time.

### Event Semantics

Memory faults publish one `EventRecord` per faulting command. The record carries queue ID, command ID, an `EventErrorKind` (`UnknownBuffer`, `RangeOverflow`, or `OutOfBounds`), and the faulting buffer ID. A successful completion is never published for the same command. Later queued commands remain runnable unless they independently fault.

### Memory-Fault Semantics

The native copy and vector-add paths use checked memory reads and writes. Bounds violations, unknown buffers, and range-overflow errors are mapped to an `EventErrorKind` and published in an `EventRecord`. The original `ModelError` propagates back through the step call so the scenario can observe it.

### Resource-Credit Semantics

`ResourceConfig` defines capacities for internal descriptors, read slots, write slots, compute slots, completion publication slots, and event publication slots. The default configuration is unbounded. `ResourceDemand` derives execution demand directly from each `ExternalCommand`: an operation with `N` native chunks or destinations demands `N` internal descriptors, `N` read slots, and `N` write slots; a vector add demands one internal descriptor, two read slots, one write slot, and one compute slot. Publication slots gate completion and event ring pushes.

### Stall Semantics

`AsyncStepStatus` reports `Progressed`, `Idle`, or `Stalled(AsyncStallReason)`. `AsyncStallReason` distinguishes execution resource pressure, completion publication pressure, event publication pressure, and a fault retirement that is still blocked on retry. Manual `SyncDevice::run_until_idle` returns `StepBudgetExhausted` when work remains after the supplied budget. Evidence: `tests/resource_credits.rs` and `tests/stall_annotations.rs`.

## Trace Sink Behavior and Chrome/Perfetto-Compatible Export

The trace sink (`model/modeling/crates/device/src/trace.rs` and submodules) captures deterministic in-memory events when enabled. `AsyncDeviceHandle` and `SyncDevice` expose `enable_tracing`, `disable_tracing`, `clear_trace`, `trace_events`, and `trace_json`.

Hook points cover command submission, doorbell/runnable transitions, scheduling, execution success, execution faults, completion/event publication and polling, IRQ source latch, line assertion/deassertion, message emission, device ack, handler start/completion, stalls, and idle steps. Public trace types include `TraceEvent`, `TraceEventKind`, `TraceOperation`, and `TraceSink`.

The JSON export is a minimal Chrome/Perfetto `Trace Event` JSON object containing a `traceEvents` array of instant events (`"ph":"i"`). The `ts` field is a deterministic sequence ordinal assigned in event-emission order, not a wall-clock timestamp or cycle count. `pid` and `tid` are placeholders. The output is compatible with existing Chrome/Perfetto viewers but does not imply real-time or cycle-level accuracy. Evidence: `tests/trace_sink.rs`.

## Determinism and Validation Strategy

Every validated slice is guarded by narrative scenario tests in `model/modeling/crates/scenarios/tests/`. The validation command set is:

- `cargo fmt --all -- --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo doc --workspace --no-deps --document-private-items`

The 56 scenario tests contain 51 modeled software programs, each with exactly one
complete `run_main` entry. Five pure device/manual observability tests have no software
actor and therefore no `run_main` entry:

- `reports_step_budget_exhaustion_when_work_remains`;
- `async_runner_no_work_reports_idle_status`;
- `async_manual_run_until_blocked_returns_terminal_status_without_losing_idle_errors`;
- `async_tracing_preserves_manual_run_until_idle_behavior`;
- `async_trace_log_records_idle_status_steps`.

Three of those five tests contain four explicit manual-driver setup calls: the manual
tracing comparison constructs separate baseline and traced drivers. The IRQ mechanism
binary contains four tests and four `run_main` calls. The standalone
`message_irq_linear_copy` test adds one complete nominal Message-IRQ path in one file:
software handler registration, configuration and submission, device completion/IRQ
emission, host delivery, handler drain/acknowledgment, and resumed-main observation.
It uses `PdmaRegisterPort<RecordingMemoryAccess>` over
`AsyncDeviceAccess` as the canonical ABI E2E. Runtime `SoftwareCpu` owns the main/handler
gate, while `PdmaIrqDelivery` starts the generic dispatcher with PDMA source/service
adapters. Ordered batches carry ACTIVATE and producer publication; Direct observations
carry queue/IRQ commands and the final raw status read. The scenario asserts the
nine-stage lifecycle through acknowledgement, vector 64, one delivery and device step,
cursor values, `MainAfterHandler`, and destination `[1, 2, 3, 4]`.
The persistent stateful-handler adapter keeps await-spanning handler-owned capabilities
in one registry entry and handler-future boxing at the host type-erasure boundary.
Cross-context test phase remains `CpuLocal`; the handler does not take and restore its
driver through a CPU-local option on every IRQ turn.
The standalone `message_irq_dual_handler` test registers separate completion and event
handlers. Main and both persistent handlers hold clones of one CPU-bound driver;
depth-one success/fault/success submissions prove shared credit reclamation across
repeated main/handler transitions while preserving independent handler call state.
Scenario bootstrap sends the driver and register-port clones to the delivery actor,
which constructs PDMA services, freezes the runtime handler registry, starts
`PdmaIrqDelivery`, and signals readiness before main enables the source. The runtime
dispatcher task owns the resulting actor loop; the one-time ownership handoff remains
private to the scenario harness and host adapter.

Actor construction, spawn, shutdown, join, trace instrumentation/collection, and final
assertions are scenario-harness responsibilities. The single intentional instrumentation
interleaving exception is
`async_trace_log_does_not_leak_disabled_submission_metadata`: trace enable/clear remains
inside its one main future so the test can order an untraced submission before a traced
doorbell without inventing a second software entry.

Each device-worker turn advances at most one native command or retirement retry. The manual `SyncDevice` facade exposes bounded `run_until_idle(max_steps)` and `run_until_blocked(max_steps)` helpers for explicit deterministic driving. Because the same inputs always produce the same runnable ordering, resource decisions, completions, events, IRQ notifications, and trace ordinals, the model is deterministic and suitable for regression testing.

## Deferred Items

The following items are explicitly outside the v0 behavioral contract:

- Cycle accuracy, RTL timing, power, or frequency modeling.
- Callable C API. The ABI-v1 data/MMIO constants header is generated and checked from
  Rust metadata, but exposes no callable interface.
- RISC-V scheduler backend through Terminus.
- High-dimensional transfer operations, gather/scatter, transpose, and reduce trees.
- Real Linux DMAengine, DPDK DMAdev, or kernel driver integration.
- Concrete PCIe/MSI-X numeric address/data transport, MSI-X table/PBA behavior, APIC/GIC
  integration, SR-IOV, FLR, or per-queue reset.

These may be addressed once the v0 mechanisms above are stable and the deferred details are validated with new scenario tests.
