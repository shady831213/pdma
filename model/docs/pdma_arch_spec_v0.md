# PDMA Active Architecture Specification

## Purpose and Scope

This document describes the active deterministic, step-based Rust model. The completed
v0.9 plan records the historical 66-register, 4 KiB MMIO design; the active successor
uses direct, dynamically addressed queue and interrupt-source registers with MMIO
version 2. ABI-v1 descriptor, completion, and event bytes remain unchanged. Historical
v0.x plans and white-board documents are background, not the current software boundary.

Validated behavior includes linear and strided copy, broadcast copy, wrapping byte-wise
vector add, multiple queues, deterministic arbitration, bounded rings, checked DMA,
fault events, resource pressure, trace export, whole-device reset, and host-owned Line
and Message IRQ delivery. Paths below are relative to `model/modeling`.

## Ownership and Source Map

| Owner | Source | Responsibility |
| --- | --- | --- |
| Shared contract | `crates/contract/src/abi/`, `crates/contract/src/semantic/` | Exact wire layouts/codecs, generated registers, identities, DMA/IRQ/resource/trace vocabulary. |
| Software SDK | `crates/host/src/request/`, `crates/host/src/config/`, `crates/host/src/semantic/driver/` | Requests, queue configuration and software owner metadata, command IDs, credit accounting, and IRQ service. |
| Software hardware adapter | `crates/host/src/adapter/abi/` | CPU-bound register port, ABI encoding/decoding, allocation handoff, memory/register access, and interrupt output adaptation. |
| Device | `crates/device/src/` | Hardware register effects, mappings, exact queues, decoded operands, arbitration, execution, retirement, reset, and IRQ state. |
| Schema | `crates/hardware-interface-schema/` | Runtime-neutral record/register metadata, codecs, composition, lookup, and exports. |
| Runtime | `crates/hardware-interface-runtime/` | Shared memory, register access/observation, CPU-local state, generic interrupt dispatch, and trace storage. |
| Pipeline | `crates/hardware-interface-pipeline/` | `Stage`, `Then`, request state, and immediate/single-step/async drive policies. |
| System harness | `crates/scenarios/tests/` | Explicit software, device-worker, and host-delivery actors plus behavioral assertions. |

The contract contains no backing memory, SDK request/configuration objects, handlers,
or executor. Host commands are encoded into ABI descriptors before hardware sees them.
The device adapter structurally decodes descriptors into private `DecodedCommand`,
`DecodedOperation`, and operand structs in `crates/device/src/operation.rs`. Operands
retain raw DMA addresses, lengths, strides, and indirect-list address/count; execution
resolves full data extents in operation order. There is one exact queue path and one
execution/retirement core. Software owner metadata stays in the host.

## Direct Hardware Register Contract

`crates/contract/src/abi/registers.rs` composes `GlobalRegisters`, `QueueRegisters`,
`IrqSourceRegisters`, `QueueInstanceRegisters`, and `PdmaRegisterMap`. Generated
layouts are the authority for exact starts, width, little-endian encoding, permissions,
reset values, masks, alignment, and geometry. There are no selector, command, result,
or response banks.

| Placement | Geometry |
| --- | --- |
| Global registers | Base 0, extent 60 bytes, alignment 8. |
| Queue instances | Base 64, stride 192 bytes. |
| Queue registers within an instance | Base 0, extent 112 bytes. |
| IRQ sources within an instance | Base 112, stride 40 bytes, count 2: completion then event. |

Use generated placements and `queue_offset`/`irq_offset`, not copied offsets. Address
arithmetic is checked against the `u32` numeric offset space. Queue instances are
materialized consecutively by configuration writes; queue count includes staged
instances. The next instance exposes default queue configuration for bootstrap.
Instance existence is device policy; repeatable geometry has no fixed queue-count
cap or 4 KiB aperture.

Queue base/depth registers describe SQ, CQ, and EQ mappings. Writing enable activates
the queue after the device validates already-committed exact mappings. Allocation
belongs to runtime provisioning. Active configuration cannot be rewritten. Software
publishes SQ producer, writes a separate doorbell to capture runnable work, and
consumes records by advancing CQ/EQ consumers. Hardware owns SQ consumer, captured
producer, and CQ/EQ producers. Route, enable, mask, ack, and status address each IRQ
source directly.

`crates/device/src/device/mmio/` routes accesses to the hardware owners. Queue and
IRQ owners consume generated values directly and do not call back into `DeviceCore`.
Global discovery projects current device state. Queue storage contains generated
configuration values and exact ring state, with no semantic ring variant.

The endpoint checks full-address base arithmetic, exact starts, widths, permissions,
and masks before effects. Each write validates its bound `DeviceEpoch` in the same
locked interaction as mutation. Non-consuming reads permit discovery across reset;
writes require explicit rebinding. A stale write cannot mutate a fresh queue at the
same numeric index.

## Ordered Host Access and Cancellation

`PdmaRegisterPort<AsyncDeviceAccess>` is the canonical software frontend. It captures
the active `SoftwareCpu` and keeps clone-shared multi-queue state in `CpuLocal`:
handles, layouts, mappings, and retained consumed records. A shared queue-operation
lock serializes its software sequences. `AsyncDeviceAccess` supplies `MemoryAccess`,
`RegisterAccess`, provisioning, and the existing generic `InterruptSource` over one
device.

Bootstrap allocates rings, writes base/depth registers, then enables the queue.
Submission writes indirect data and descriptor bytes before publishing SQ producer;
doorbell is later. Each allocation, memory write, and register write commits
independently. Dropping a future stops subsequent accesses and leaves completed
effects intact, including partial allocation/configuration. Hardware receives no
software-cancellation input and implements no cross-memory/MMIO transaction.

Every nested CPU-bound create, submit, doorbell, completion, and event port poll first
validates the owning active CPU. Migration to inactive or wrong-CPU execution returns
a typed error before another hardware access. No accounting or CPU-local borrow spans
a poll or await.

The driver reserves a slot and consumes a command ID before submission. Its shared
receipt marks acceptance synchronously when SQ producer publication commits. Rejection
before acceptance permits deferred slot refund; the command ID remains consumed.
Doorbell failure/cancellation cannot refund accepted work or invent a missing doorbell.

Polling reads a record before advancing its consumer. After that write commits, the
port retains the decoded record until returning it once; driver accounting reclaims
the slot synchronously with that return. Empty polling does not advance a consumer.

Observation order is reserved before mutation and emitted after unlocking.
`CommittedObservation` distinguishes a write that took effect but whose observation
failed from a rejected access. Acceptance and retained-record bookkeeping honor that
distinction. Observation failure does not roll hardware back. Reset requires a newly
bound port and driver.

## Device Composition and Execution

`DeviceCore` in `crates/device/src/device/mod.rs` owns epoch, checked mapping metadata,
queues, runnable FIFO, issue/execution/retirement operators, PDMA IRQ state, and trace.
Bytes live in an injected runtime-owned memory backend. Runtime provisioning owns
address allocation and backing installation; external mappings must use that same
backend. `QueueBlock` owns configuration and exact SQ/CQ/EQ state. The host writes
SQ bytes and hardware writes CQ/EQ bytes in shared memory.

`IssueUnit` owns `Then<Arbitration, Fetch>`. Arbitration considers at most one pass
through the runnable FIFO, rotates resource-starved queues, and chooses the first
eligible queue before fetching. A fully starved pass preserves ordering and emits
one stall. Probes observe without selecting, mutating, or tracing. Fetch decodes an
exact descriptor into private operands or a structural ABI frontend fault. Payload
memory validation belongs to execution, not descriptor decoding.

`ExecutionUnit` owns `Then<ResourceGate, Datapath>`; its gate rechecks resource demand
at the execution commit boundary. Owned edges in `crates/device/src/device/flow.rs`
carry fetched, admitted, and retirement work. DMA faults travel as data into
retirement; structural model errors can terminate a request.

Normal work follows `Then<IssueUnit, Then<ExecutionUnit, RetirementUnit>>` over a
scoped `HardwareContext`. Operators remain outside that context and borrow shared
hardware state only during interactions. Queue and runnable state are not stages.
`Stage` separates hardware from request state; `ThenState` retains downstream
requests, and `Operation` makes completed/reset requests inert without replacing
hardware.

The device retains a normal composed request or a retirement request using the same
`RetirementUnit`. A frontend `FaultCached` ends one request; only a later request
retries publication before fresh issue admission. Publication pressure retains work
without re-arbitration, refetch, repeated DMA, or duplicated effects. Each outer turn
permits one architecture-visible interaction: selection/fetch, resource check/DMA, or
retirement. Public step accounting advances only at the terminal command outcome.

The private runtime serializes host access and worker interactions with one Tokio
lock. Its worker uses the generic `AsyncExecutor` and `DeviceInteractionBackend`;
`DeviceService` adapts `Stage<()>` to continuous service, accounting, idle/stall wake
tokens, and shutdown. Accepted work stays in the core when a driving future is dropped.
Hardware owners contain no async lock, observer, or host callback. Memory backend
entry occurs beneath the device interaction, never in reverse lock order; guards are
released before waiting.

## Operations, Faults, and Resource Pressure

Execution resolves full ranges through `crates/device/src/device/execution/memory.rs`.
Linear copy reads its source and validates its destination before writing. Strided
copy validates every row's source/destination before ordered row transfers. Broadcast
validates the source, reads the indirect list, and validates every destination before
ordered writes, rereading the source per destination. Vector add reads both sources
and validates the destination before writing byte-wise wrapping sums. These checks
preserve operation-specific fault precedence and prevent writes before a detected
extent fault; a later backend access fault can still leave earlier writes committed.

One successful command retires a hardware `AbiCompletion`. Execution faults carry
the exact `AbiFault` plus optional semantic memory-fault metadata for model reporting;
event publication preserves the exact fault even when no mapped host object exists.
A fault never publishes a success completion for that command. Later queued commands
remain runnable as required by the scenarios.

`ResourceConfig` defines descriptor, read, write, compute, completion-publication,
and event-publication capacities. N copy chunks/destinations demand N descriptors,
reads, and writes; vector add demands one descriptor, two reads, one write, and one
compute slot. Defaults are unbounded. Publication must satisfy both ring space and
configured publication capacity: outstanding occupancy consumes publication slots
until software advances the consumer. A nonzero capacity is not unconditional
permission to publish into a deeper ring.

`AsyncStepStatus` distinguishes progressed, idle, and stalled work.
`AsyncStallReason` distinguishes execution, completion-publication,
event-publication, and fault-retirement retry pressure. Pending publication resumes
without repeating execution.

## Whole-Device Reset

Reset checks the expected epoch and checked-advances the canonical `DeviceEpoch`.
Stale reset and epoch overflow leave state unchanged. Success clears queue
configuration/cursors, runnable work, composed requests, pending retirement, IRQ
routes/causes, and queued notifications. It preserves committed mapping metadata and
generation, bytes and allocation state, trace history, resource configuration, and
lifecycle accounting.

Old queue, IRQ, notification, and register-write capabilities reject before mutation.
Discovery reads permit explicit bootstrap in the new epoch. Reset clears accepted
pending work before later completion, success trace, consumer change, or IRQ
publication; already-committed DMA bytes remain.

## IRQ and CPU Ownership

Device-local source enable, mask, route, pending cause, and acknowledgement are
hardware state. Masking preserves pending work. Line acknowledgement clears a cause
only when its completion/event path is empty; publication after software's final empty
poll keeps the cause pending. Message routing retains each representable trigger with
its vector, source, and source-scoped monotonic message ID, without intentional
coalescing or physical MSI/MSI-X table/address/data behavior.

`AsyncDeviceAccess` implements runtime `InterruptSource` directly. Readiness is
non-consuming; claim consumes hardware notification output with epoch validation.
The register port controls sources and returns typed snapshot errors; it does not
duplicate notification wait/claim methods. There is no MMIO claim protocol or host
IRQ coordinator. `PdmaIrqDelivery` supplies
PDMA service, trace, and error adaptation to the generic `InterruptDispatcher`.
The device never stores handlers or invokes software.

The host freezes a non-empty vector registry before delivery starts. Each vector owns
one persistent handler; multiple sources may share it. Duplicates do not replace
handlers. Unregistered vectors fail before CPU acquisition, lifecycle tracing, or
successful-delivery accounting. Shutdown wins over simultaneously ready notification.

Every software-bearing scenario runs one complete main future through
`SoftwareCpu::run_main`. One main segment is one actual poll; only `Poll::Pending`
ends a segment before reacquisition. Immediately-ready awaits create no interrupt
boundary. A complete handler future holds the CPU gate across its polls and awaits.
Handlers are serial and non-reentrant; main cannot overlap them, while the device
worker can progress.

CPU ordering is FIFO after a contender requests the gate. Unobserved notifications
have no priority, and a later IRQ request cannot overtake queued main work.
`CpuLocal::with` accesses a single CPU-bound value synchronously, exposing no identity,
guard, or await-spanning borrow. The private token occupies one
`CURRENT_SOFTWARE_CPU` option around each inner poll; RAII restores its predecessor,
including nested contexts. Persistent `StatefulIrqHandler` state owns handler-private
resources across awaits.

`AsyncDriver` setup captures the active CPU; clones share command IDs and accounting.
It alone services IRQs, draining records before acknowledgement. `ManualDriver`
remains a focused host observability API with no IRQ service; the canonical port is
always CPU-bound. System observability exceptions use the raw harness fixture.

IRQ bootstrap retains a driver in main and sends a clone plus port/source
capabilities to delivery. Delivery registers handlers and signals readiness before
main configures/enables sources or submits. No handler waits for initialization while
owning the CPU. The depth-one dual-handler scenario validates shared credit
reclamation across success/fault/success turns.

## Data ABI, Trace, and Validation

ABI-v1 is fixed-width little-endian data, not Rust layout: descriptors are 64 bytes,
completions 32 bytes, and events 64 bytes. `DmaAddress` is opaque. The generated
`include/pdma_model_abi.h` contains data and direct MMIO constants, not callable APIs.
Never edit it by hand.

Hardware trace records actual transitions. SQ producer advancement emits submission
events for newly published decodable descriptors; CQ/EQ consumer advancement emits
record-consumption events. Unchanged cursor writes and empty software polling emit no
hardware submission/consumption trace. Disabled tracing retains no metadata for later
emission. Other hooks cover runnable/scheduling, execution, publication, stalls, idle,
IRQ state, and handler lifecycle. Chrome/Perfetto JSON uses instant events with
deterministic sequence ordinals, not wall-clock timestamps or cycles.

The canonical E2E is `crates/scenarios/tests/message_irq_linear_copy/mod.rs`, using
`PdmaRegisterPort<RecordingMemoryAccess>` over `AsyncDeviceAccess`. It observes direct
enable/producer/consumer/IRQ writes, descriptor memory traffic, main-handler-main,
vector 64, completion, acknowledgement, and destination bytes `[1, 2, 3, 4]`.
`crates/scenarios/tests/message_irq_dual_handler/mod.rs` covers separate persistent
completion/event handlers. Worker and delivery shutdown/join precede collection.

Every software-bearing test has exactly one `run_main`. These five device-only
observability tests have none and use raw hardware setup where needed:

- `reports_step_budget_exhaustion_when_work_remains`;
- `async_runner_no_work_reports_idle_status`;
- `async_manual_run_until_blocked_returns_terminal_status_without_losing_idle_errors`;
- `async_tracing_preserves_manual_run_until_idle_behavior`;
- `async_trace_log_records_idle_status_steps`.

The fixture is `crates/scenarios/tests/support/raw_hardware.rs`. Actor construction,
spawn, shutdown, join, tracing, and final assertions belong to the harness. The sole
trace-control exception is `async_trace_log_does_not_leak_disabled_submission_metadata`,
which enables/clears tracing inside its one main future to preserve the interleaving.
Additional regression scenarios retain the same one-entry/five-exception rule.

Run from `model/modeling`:

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps --document-private-items
cargo run -p pdma-model-contract --bin pdma-abi-header -- --check include/pdma_model_abi.h
```

Regenerate with the same header command using `--write` instead of `--check`. Native
Rust module layout, sub-250 pure-LOC files, checked memory, and one execution/retirement
core remain binding. Cycle timing, callable C APIs, Terminus, physical PCIe/MSI-X maps,
SR-IOV, FLR, per-queue reset, and real OS integration remain deferred.
