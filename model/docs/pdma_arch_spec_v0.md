# PDMA v0 Architecture Specification

## Purpose and Scope

This document records the validated behavioral contract of the PDMA executable model through v0.7. It is the source of truth for what the Rust model proves today. White-board architecture documents are background only. The spec deliberately defers anything that is not exercised by the current scenario tests, including cycle timing, descriptor byte layouts, register offsets, the C ABI, generated headers, concrete PCIe MSI/MSI-X transport, reset behavior, and real operating-system integration.

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
- Host-owned IRQ delivery with one registered handler and a CPU token shared with SW main.
- One complete software-main future whose real async suspension points define interruptible
  execution boundaries.

The contract is deterministic: the same initial state, resource configuration, arbitration policy, and command sequence always produces the same memory contents, completion records, event records, and trace event order.

## Software/Driver Responsibility Boundary

The driver model, implemented in `model/modeling/crates/host/src/driver.rs`, owns the following responsibilities:

- Allocate device-visible buffers through `AsyncDevicePort` and fill source data through `write_buffer_on`.
- Configure a queue with `AsyncDriver::setup_on` or `setup_with_config_on`, receiving a `QueueConfigHandle`.
- Assign monotonic per-queue command IDs and track free local command slots.
- Build external request structs (`CopyRequest`, `StridedCopyRequest`, `BroadcastCopyRequest`, `VectorAddRequest`) and call the matching `submit_*` helper.
- Ring the implicit doorbell by submitting through the driver helper.
- Poll the bound completion ring with `poll_completion` and reclaim one local slot per returned record.
- Poll the queue-local event path with `poll_event_on` and reclaim one local slot per returned event.
- Configure device IRQ routes and source controls through the host device port.
- Demultiplex IRQ sources, drain the relevant completion/event path, then acknowledge the device cause.

The driver does not access the command ring directly, does not execute commands, and does not rely on wall-clock timing or cycle counts.

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
cycle timing and does not model reset.

## Queue Directory, Queue Configuration, Command Ring, Completion Ring, and Event Ring Ownership

Queue directory entries (`QueueDirectoryEntry` in `model/modeling/crates/contract/src/queue.rs`) bind together:

- A `QueueId` assigned by the device.
- A bound `CompletionRingId`.
- A `QueueConfig` supplied by the driver, containing command ring depth, completion ring depth, event routing policy, and a software owner ID.

The device owns the storage for all command rings, completion rings, and queue-tagged events. The driver owns the returned `QueueConfigHandle`, which is the only driver-visible token for that queue and completion ring. Local driver slot accounting is a driver-side shadow; the device never tracks how many commands the driver thinks are outstanding.

For event routing, v0 supports only `EventRouting::QueueLocal`. A driver may poll only events whose `queue_id` matches its own queue through `poll_queue_event`.

## Submission, Doorbell/Runnable, Scheduling, Execution, Completion, and Event Flows

### Submission

The driver builds a request, wraps it in an external command descriptor, and calls `submit_*`. The driver decrements its free slot count, assigns the next command ID, pushes the command onto the command ring, and rings the doorbell. A `TraceEventKind::CommandSubmitted` event is emitted.

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

The host-owned `IrqDelivery` actor registers one software handler through type state and
never places a handler in the device. `SoftwareCpu::run_main` receives one complete main
future. One main atomic segment is exactly one poll of that future, ending in
`Poll::Ready` or `Poll::Pending`; only a real `Poll::Pending` releases the CPU for later
main reacquisition. Consequently, an `.await` whose awaited future is immediately ready
does not create an interrupt boundary.

The complete handler future owns the same CPU token across all of its polls and
suspensions. Main and IRQ contexts therefore do not overlap, handlers are atomic and
non-nested, and the independent device worker may still progress. CPU ordering follows
the Tokio mutex's FIFO request order: an IRQ precedes main reacquisition only after the
delivery actor has observed a notification and requested the gate first. An unobserved
device notification has no CPU priority, and a later IRQ request does not overtake an
already queued main request; the model makes no broader scheduler-priority promise.

Driver IRQ service reads the delivered source, drains completion/event records, then
acknowledges the device cause. This ordering is idempotent for redundant message
deliveries, and the device retains a Line cause if a new record appears before ack.
Shutdown has deterministic priority over a simultaneously ready notification. Delivery
progress is synchronized by handler-owned channels when a scenario needs it, while the
actor's final `IrqDeliverySummary` reports only successfully completed handler turns; no
separate delivery-progress watch API exists. Evidence:
`crates/device/tests/irq_line_ack.rs`, `crates/host/tests/irq_delivery.rs`, and
`crates/scenarios/tests/irq_delivery.rs`.

## Conceptual Command Boundary

External commands live in `model/modeling/crates/contract/src/desc.rs` and represent the conceptual driver-visible descriptor surface. The `ExternalCommand` enum carries `Copy`, `StridedCopy`, `BroadcastCopy`, and `VectorAdd` variants, each with queue ID, completion ring ID, command ID, and request payload. The active native runner matches these variants exhaustively and derives operation-specific chunks or resource demand internally. Exact serialized descriptor layout remains deferred, so these Rust values define behavior rather than an ABI.

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

The 52 scenario tests contain 47 modeled software programs, each with exactly one
complete `run_main` entry. Five pure device/manual observability tests have no software
actor and therefore no `run_main` entry:

- `reports_step_budget_exhaustion_when_work_remains`;
- `async_runner_no_work_reports_idle_status`;
- `async_manual_run_until_blocked_returns_terminal_status_without_losing_idle_errors`;
- `async_tracing_preserves_manual_run_until_idle_behavior`;
- `async_trace_log_records_idle_status_steps`.

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
- Exact descriptor byte layout and register offsets.
- C ABI and generated headers.
- RISC-V scheduler backend through `terminus`.
- High-dimensional transfer operations, gather/scatter, transpose, and reduce trees.
- Real Linux DMAengine, DPDK DMAdev, or kernel driver integration.
- Concrete PCIe MSI/MSI-X transport, APIC/GIC integration, or device reset behavior.

These may be addressed once the v0 mechanisms above are stable and the deferred details are validated with new scenario tests.
