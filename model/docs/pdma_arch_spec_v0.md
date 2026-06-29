# PDMA v0 Architecture Specification

## Purpose and Scope

This document records the validated behavioral contract of the PDMA v0 executable model. It is the source of truth for what the Rust model proves today. White-board architecture documents are background only. The spec deliberately defers anything that is not exercised by the current scenario tests, including cycle timing, descriptor byte layouts, register offsets, the C ABI, generated headers, interrupts, reset behavior, and real operating-system integration.

The intended readers are implementers who will extend the model, port it to a cycle-accurate or RTL context, or write future drivers against a more concrete ABI.

## Validated v0 Model Contract

The v0 model validates the following end-to-end behaviors:

- Typed object identifiers for queues, commands, buffers, and completion rings.
- Checked device memory with explicit bounds and range-overflow errors.
- Bounded command, completion, and event rings with full/empty/wrap behavior.
- Driver queue setup, submission, doorbell, completion polling, and event polling.
- Device step loop that advances at most one command per `step()` and reports budget exhaustion in `run_until_idle()`.
- Behavioral scheduler lowering from external commands to internal scheduled work.
- Linear copy, 2D strided copy, broadcast copy, and byte-wise wrapping vector add.
- Queue directory and explicit per-queue configuration objects.
- Deterministic multi-queue arbitration with round-robin starvation avoidance.
- Recoverable memory-fault events that skip success completions.
- Resource-credit gating and observable stall annotations.
- Deterministic trace sink with structured in-memory events and Chrome/Perfetto JSON export.

The contract is deterministic: the same initial state, resource configuration, arbitration policy, and command sequence always produces the same memory contents, completion records, event records, and trace event order.

## Software/Driver Responsibility Boundary

The driver model, implemented in `model/modeling/crates/host/src/driver.rs` after the v0.5 workspace split, owns the following responsibilities:

- Allocate device-visible buffers through `PdmaDevice::allocate_buffer` and fill source data through `write_buffer`.
- Configure a queue with `Driver::setup` or `Driver::setup_with_config`, receiving a `QueueConfigHandle`.
- Assign monotonic per-queue command IDs and track free local command slots.
- Build external request structs (`CopyRequest`, `StridedCopyRequest`, `BroadcastCopyRequest`, `VectorAddRequest`) and call the matching `submit_*` helper.
- Ring the implicit doorbell by submitting through the driver helper.
- Poll the bound completion ring with `poll_completion` and reclaim one local slot per returned record.
- Poll the event path with `poll_event`, or queue-local events with `poll_queue_event`, and reclaim one local slot per returned event.

The driver does not access the command ring directly, does not execute commands, and does not rely on wall-clock timing or cycle counts.

## Device Responsibility Boundary

The device model, implemented in `model/modeling/crates/device/src/device.rs` after the v0.5 workspace split, owns the following responsibilities:

- Maintain device memory, the queue directory, command rings, completion rings, the shared event ring, the runnable queue list, the arbiter, the resource pool, and the trace sink.
- Accept submissions through `submit_command`, emit a trace event, and queue the command on the correct command ring.
- Capture doorbells through `ring_doorbell` and mark queues runnable.
- Select the next queue through the configured arbitration policy.
- Lower the selected external command into internal scheduled work through `Scheduler::lower`.
- Acquire execution resources, run the work through the copy or vector-add engine, and release resources.
- Publish success completions to the bound completion ring, or publish memory-fault events to the event ring.
- Stall deterministically when resources or publication slots are exhausted, and retry pending retirements on later steps.
- Expose tracing controls and exported JSON without changing behavior when tracing is disabled.

The device does not require an async runtime, does not expose wall-clock or cycle timing, and does not model interrupts or reset.

## Queue Directory, Queue Configuration, Command Ring, Completion Ring, and Event Ring Ownership

Queue directory entries (`QueueDirectoryEntry` in `model/modeling/crates/contract/src/queue.rs`) bind together:

- A `QueueId` assigned by the device.
- A bound `CompletionRingId`.
- A `QueueConfig` supplied by the driver, containing command ring depth, completion ring depth, event routing policy, and a software owner ID.

The device owns the storage for all command rings, completion rings, and the shared event ring. The driver owns the returned `QueueConfigHandle`, which is the only driver-visible token for that queue and completion ring. Local driver slot accounting is a driver-side shadow; the device never tracks how many commands the driver thinks are outstanding.

For event routing, v0 supports only `EventRouting::QueueLocal`. A driver may poll only events whose `queue_id` matches its own queue through `poll_queue_event`. Global event polling through `poll_event` returns events in publication order without filtering.

## Submission, Doorbell/Runnable, Scheduling, Execution, Completion, and Event Flows

### Submission

The driver builds a request, wraps it in an external command descriptor, and calls `submit_*`. The driver decrements its free slot count, assigns the next command ID, pushes the command onto the command ring, and rings the doorbell. A `TraceEventKind::CommandSubmitted` event is emitted.

### Doorbell/Runnable

`ring_doorbell` adds the queue to the runnable list if it is not already present. A `TraceEventKind::DoorbellRunnable` event is emitted.

### Scheduling

`step_with_status` asks the arbiter to pick the next runnable queue. If none exists, the device records `TraceEventKind::Idle` and returns `StepStatus::Idle`. If a queue is selected, the command at the front of its command ring is cloned and passed to `Scheduler::lower`, producing `ScheduledWork`. A `TraceEventKind::Scheduled` event is emitted.

### Execution

The device computes the resource demand for the scheduled work. If resources are insufficient, the queue is re-queued as runnable, a `TraceEventKind::Stall` event is emitted, and `StepStatus::Stalled(ExecutionResourcePressure)` is returned. Otherwise the resources are acquired and the engine runs.

On success, `TraceEventKind::ExecutionSucceeded` is emitted. On a memory fault, `TraceEventKind::ExecutionFaulted` is emitted and the fault is routed to the event path.

### Completion

A successful command produces one `CompletionRecord` with `CompletionStatus::Success`. If the completion ring is full or publication credits are exhausted, the retirement becomes pending, a stall event is emitted, and the device returns `StepStatus::Stalled(CompletionPublicationPressure)`. When publication succeeds, `TraceEventKind::CompletionPublished` is emitted. The driver later calls `poll_completion`, which emits `TraceEventKind::CompletionPolled`.

### Event

A memory fault produces one `EventRecord` with an `EventErrorKind`. If event publication credits are exhausted, the retirement becomes pending, a stall event is emitted, and the device returns `StepStatus::Stalled(EventPublicationPressure)` or `FaultRetirementPendingRetry`. When publication succeeds, `TraceEventKind::EventPublished` is emitted and the original `ModelError` is returned to the caller. The driver polls the event path with `poll_event` or `poll_queue_event`, emitting `TraceEventKind::EventPolled`.

## External Command versus Internal Scheduled Work Boundary

External commands live in `model/modeling/crates/contract/src/desc.rs` and represent the driver-visible descriptor surface. The `ExternalCommand` enum currently carries `Copy`, `StridedCopy`, `BroadcastCopy`, and `VectorAdd` variants, each with queue ID, completion ring ID, command ID, and request payload.

The scheduler (`model/modeling/crates/device/src/scheduler.rs`) lowers each external command into internal scheduled work defined in `model/modeling/crates/device/src/scheduled.rs`:

- Copy commands become `ScheduledWork::Copy(ScheduledCopy)`, which contains a vector of `CopyChunk` row descriptors.
- Vector-add commands become `ScheduledWork::VectorAdd(ScheduledVectorAdd)`.

Engines operate only on scheduled work. This boundary lets the external descriptor layout evolve without changing execution semantics.

## Supported v0 Operations

### Linear Copy

`CopyRequest` in `desc.rs` takes a source buffer, destination buffer, and byte length. The scheduler lowers it to one `CopyChunk` with zero offsets. Evidence: `tests/linear_copy.rs`.

### Strided Copy

`StridedCopyRequest` takes source and destination buffers, row count, row bytes, and source/destination strides. The scheduler emits one chunk per row. Stride multiplication is checked for overflow and maps to `RangeOverflow`. Evidence: `tests/strided_copy.rs`.

### Broadcast Copy

`BroadcastCopyRequest` takes a source buffer with offset and length plus a list of destination ranges. The scheduler emits one chunk per destination. Execution stops on the first memory fault and publishes a single event; otherwise a single success completion is published. Evidence: `tests/broadcast_copy.rs`.

### Vector Add

`VectorAddRequest` takes two source buffer ranges and one destination range plus a length. The vector-add engine reads both source ranges, performs byte-wise wrapping addition, and writes the result. Evidence: `tests/vector_add.rs`.

## Completion, Event, Memory-Fault, Resource-Credit, and Stall Semantics

### Completion Semantics

A success completion is one `CompletionRecord` per successful command. It carries the queue ID, command ID, and `CompletionStatus::Success`. Completions are published to the completion ring bound at queue creation time.

### Event Semantics

Memory faults publish one `EventRecord` per faulting command. The record carries queue ID, command ID, an `EventErrorKind` (`UnknownBuffer`, `RangeOverflow`, or `OutOfBounds`), and the faulting buffer ID. A successful completion is never published for the same command. Later queued commands remain runnable unless they independently fault.

### Memory-Fault Semantics

The copy and vector-add engines use checked memory reads and writes. Bounds violations, unknown buffers, and range-overflow errors are converted to `MemoryFault` and then to an `EventRecord`. The original `ModelError` propagates back through the step call so the scenario can observe it.

### Resource-Credit Semantics

`ResourceConfig` defines capacities for internal descriptors, read slots, write slots, compute slots, completion publication slots, and event publication slots. The default configuration is unbounded. Execution demand is derived from the scheduled work: a copy with `N` chunks demands `N` internal descriptors, `N` read slots, and `N` write slots; a vector add demands one internal descriptor, two read slots, one write slot, and one compute slot. Publication slots gate completion and event ring pushes.

### Stall Semantics

`StepStatus` reports `Progressed`, `Idle`, or `Stalled(StallReason)`. `StallReason` distinguishes execution resource pressure, completion publication pressure, event publication pressure, and a fault retirement that is still blocked on retry. `run_until_idle` returns `StepBudgetExhausted` when work remains after the supplied budget. Evidence: `tests/resource_credits.rs` and `tests/stall_annotations.rs`.

## Trace Sink Behavior and Chrome/Perfetto-Compatible Export

The trace sink (`model/modeling/crates/device/src/trace.rs` and submodules) captures deterministic in-memory events when enabled. `PdmaDevice` exposes `enable_tracing`, `disable_tracing`, `clear_trace`, `trace_events`, and `trace_json`.

Hook points cover command submission, doorbell/runnable transitions, scheduling, execution success, execution faults, completion publication, event publication, completion polling, event polling, stalls, and idle steps. Public trace types include `TraceEvent`, `TraceEventKind`, `TraceOperation`, and `TraceSink`.

The JSON export is a minimal Chrome/Perfetto `Trace Event` JSON object containing a `traceEvents` array of instant events (`"ph":"i"`). The `ts` field is a deterministic sequence ordinal assigned in event-emission order, not a wall-clock timestamp or cycle count. `pid` and `tid` are placeholders. The output is compatible with existing Chrome/Perfetto viewers but does not imply real-time or cycle-level accuracy. Evidence: `tests/trace_sink.rs`.

## Determinism and Validation Strategy

Every validated slice is guarded by narrative scenario tests in `model/modeling/crates/scenarios/tests/`. The validation command set is:

- `cargo fmt --all -- --check`
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo doc --workspace --no-deps --document-private-items`

`PdmaDevice::step()` advances at most one command. `run_until_idle(max_steps)` repeats `step()` until no work remains or the budget is exhausted. Because the same inputs always produce the same sequence of arbitration decisions, resource acquisitions, completions, events, and trace ordinals, the model is deterministic and suitable for regression testing.

## Deferred Items

The following items are explicitly outside the v0 behavioral contract:

- Cycle accuracy, RTL timing, power, or frequency modeling.
- Exact descriptor byte layout and register offsets.
- C ABI and generated headers.
- RISC-V scheduler backend through `terminus`.
- High-dimensional transfer operations, gather/scatter, transpose, and reduce trees.
- Real Linux DMAengine, DPDK DMAdev, or kernel driver integration.
- Interrupt delivery, MSI-X, or device reset behavior.

These may be addressed once the v0 mechanisms above are stable and the deferred details are validated with new scenario tests.
