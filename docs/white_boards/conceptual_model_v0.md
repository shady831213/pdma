# PDMA Conceptual Model v0 Proposal

## Purpose

This document proposes a Rust-based conceptual model for PDMA. The goal is to create a discussion starting point before any implementation work begins.

The model is intended to explore and refine:

- Software and hardware component partitioning.
- External command descriptor and internal execution descriptor boundaries.
- Queue, ring, queue group, and scheduler interfaces.
- Control path and data path interaction.
- Optional compute pipeline attachment and bypass behavior.
- Completion, event, interrupt, error, and tracing semantics.
- The shape of a future SDK, driver model, and verification reference model.

## Non-Goals

This first model is not intended to be:

- Cycle accurate.
- RTL timing accurate.
- A throughput or power signoff model.
- A final driver ABI definition.
- A final register map definition.
- A complete replacement for subsystem verification.

It should be executable enough to test architectural ideas, but abstract enough to change quickly.

## Agreed v0 Direction

The current v0 direction is:

- Use Rust for the conceptual model.
- Use a deterministic discrete-event or step-based top-level execution model with `run_until_idle()`-style scenario tests.
- Keep component boundaries message-oriented and async-friendly, but do not require a real async runtime in v0.
- Model memory types explicitly: system/external memory, DMA-coherent ring memory, PDMA private local storage, and PDMA data-path buffers.
- Make queue directory, queue configuration, command rings, completion rings, event rings, producer/consumer indices, phase/wrap handling, and memory placement relatively concrete from the beginning.
- Keep command descriptor byte layout, register byte offsets, C ABI, and generated headers abstract in v0, while documenting that they must be finalized before the model becomes a deliverable interface reference.
- Focus first performance exploration on bandwidth and backpressure rather than realistic workload coverage.
- Include three data-path choices in v0: bypass copy, a simple transform engine, and a vector-add engine.
- Use completion-based command ring reclamation first. Do not require hardware to update queue configuration consumer/progress fields in the submit fast path.
- Model the scheduler as a behavioral scheduler first. Leave room for a later RISC-V ISA scheduler backend using `terminus`.
- Keep the initial white-board notes under `pdma/docs/white_boards`, then move stable modeling documentation under `pdma/model/docs` when implementation begins.
- Plan future Rust implementation under `pdma/model/modeling`.

The modeling deliverable should eventually make queue/config/ring structures, descriptor layouts, register layouts, and generated headers precise. v0 only needs to establish their fields, ownership, and interaction strongly enough to guide discussion.

### v0 Decision Rationale

| v0 decision | Concrete reason | Reference pattern | Tradeoff or deferred consequence |
| --- | --- | --- | --- |
| Use Rust for the conceptual model. | The model needs strong typed IDs, ownership of rings and buffers, deterministic tests, and fast iteration without committing to RTL or C ABI layouts. | `etha_model` in [Reference Index](../references/README.md), Rust type-system practice. | Rust model types are not automatically the final driver ABI; generated C headers are deferred. |
| Start with deterministic `step()` and `run_until_idle()` scenario tests. | Architecture discussions need reproducible state transitions and small failing scenarios before concurrency and timing effects are introduced. | `etha_model`, hardware functional-model practice. | Natural producer-consumer concurrency may be approximated until an async or timed model is added. |
| Keep message-oriented component boundaries but avoid a real async runtime in v0. | Scheduler/engine interfaces should remain close to the eventual hardware partition, while the first model must stay easy to debug and test deterministically. | Linux DMAengine and DPDK DMAdev queue/descriptor flows in [Reference Index](../references/README.md). | Backpressure and timing interactions are modeled explicitly later rather than by runtime scheduling behavior. |
| Model external memory, coherent rings, local scheduler storage, and data-path buffers as separate concepts. | Queue ownership, command visibility, completion visibility, and payload movement each have different correctness rules. | virtio, NVMe, and RDMA verbs in [Reference Index](../references/README.md). | The first model abstracts caches and SoC memory ordering; integration-specific coherency rules remain deferred. |
| Make queue directory, queue config, rings, indices, and phase/wrap behavior concrete early. | Queue correctness bugs are interface bugs; they must be visible before adding compute, dependencies, or performance annotations. | virtio, NVMe, and RDMA verbs in [Reference Index](../references/README.md). | Byte-accurate descriptor and register layout can still remain abstract until the fields stabilize. |
| Keep descriptor byte layout, register offsets, C ABI, and generated headers abstract in v0. | Freezing layout too early would turn exploratory model code into a compatibility burden before architecture behavior is validated. | `etha_model` and Linux DMAengine provider references in [Reference Index](../references/README.md). | A future spec draft must explicitly mark which layouts are stable before software integration can depend on them. |
| Start performance work with bandwidth, backpressure, and stall annotations rather than cycle accuracy. | Early architecture questions are about likely bottlenecks and component budget pressure, not signoff timing. | Perfetto/Chrome trace references in [Reference Index](../references/README.md). | The model cannot predict RTL frequency, arbitration latency, or NoC timing until lower-level design data exists. |
| Include bypass copy, transform, and vector-add-style compute choices. | These cover pure DMA, single-source compute, and multi-source synchronization without becoming a general tensor/dataflow framework. | Linux DMAengine memory-to-memory capabilities; Intel DSA as a data-movement accelerator contrast. | Exact final compute ISA, fixed-function operation set, and programmable-kernel model remain integration decisions. |
| Use completion-based command ring reclamation first. | It is conservative, simple to prove in tests, and avoids adding a descriptor-consumed progress interface before the ring contract is stable. | Linux DMAengine client, RDMA verbs, and NVMe references in [Reference Index](../references/README.md). | Earlier descriptor reuse may require a consumed-index or SQ-head-like mechanism if command ring pressure becomes important. |
| Model the scheduler as behavioral first and leave `terminus` for later. | The immediate question is scheduler responsibility and state, not instruction binary compatibility or firmware cycle cost. | `terminus` and `terminus_cosim` in [Reference Index](../references/README.md). | Firmware/ISA behavior, microcode cost, and co-simulation are deferred until the scheduler implementation style is chosen. |
| Keep stable docs under `pdma/model/docs` only after model behavior is validated. | White-board decisions should not masquerade as a stable architecture contract before tests exercise them. | Spec-driven development practice, NVMe/virtio style separation between overview and normative structure. | A post-v0 `pdma_arch_spec_v0.md` must reconcile model behavior with eventual ABI/register definitions. |

## Reference Style

The existing `etha_model` project is a useful reference because it demonstrates Rust hardware functional modeling with:

- A Rust library as the model core.
- C examples through FFI.
- Register and descriptor header generation.
- Multi-ring and multi-queue device behavior.
- Unit tests with `cargo test`.
- Chrome tracing JSON output.
- Python trace analysis helpers.

PDMA should borrow the useful ideas, but not copy the structure blindly. Some parts may be better expressed with more explicit async/event-driven modeling, stricter typed interfaces, and a cleaner separation between conceptual architecture objects and C ABI objects.

The shared reference catalog for specs, codebases, and research leads lives in [PDMA Reference Index](../references/README.md). Inline references in this document should explain why a source matters for a decision rather than merely naming the source.

## Modeling Philosophy

The model should treat PDMA as a set of interacting components rather than one monolithic device function. Each component should expose a narrow interface that mirrors the architectural boundary we may later use in RTL, firmware, driver, or SDK work.

The model should favor:

- Strong Rust types for IDs, descriptors, queue entries, status codes, and events.
- Explicit ownership of rings, memory regions, and internal state.
- Deterministic execution for tests by default.
- Optional async execution when it clarifies component interaction.
- Traceable state transitions.
- Small scenario tests that explain architecture decisions.

The model should avoid:

- Encoding RTL microarchitecture too early.
- Making all state globally mutable.
- Treating software-visible descriptors and internal descriptors as the same structure.
- Hiding queue arbitration or dependency handling inside ad hoc helper functions.

## Proposed Modeling Layers

```text
application scenario / test
  -> SDK-like builder API
  -> driver model
  -> memory and ring model
  -> PDMA device model
       -> command queue manager
       -> descriptor fetcher
       -> scheduler coprocessor model
       -> queue groups
       -> DMA read engine model
       -> optional compute pipeline model
       -> DMA write engine model
       -> completion/event/interrupt model
  -> trace and assertions
```

### SDK-like Layer

The SDK-like layer is not the final SDK. It is a modeling convenience for constructing logical tasks and task graphs.

It should build:

- Copy tasks.
- Compute-assisted copy tasks.
- Fence and barrier tasks.
- Small task graphs with explicit dependencies.
- Queue selection and queue group hints.

Output from this layer should still pass through a driver model, so the model preserves the software/hardware split.

### Driver Model

The driver model represents the software that owns queue setup, descriptor emission, doorbells, completion polling, and event handling.

It should model:

- Queue set allocation.
- Command, completion, and event ring placement.
- Memory handle validation as an external assumption.
- Translation from SDK logical tasks into external command descriptors.
- Doorbell writes.
- Completion and event consumption.

It should not model real Linux kernel APIs or DPDK PMD internals in v0. Instead, it should expose enough structure to later map to Linux DMAengine, DPDK DMAdev, and the advanced SDK.

### Memory And Ring Model

The memory model should distinguish object placement rather than emulate a full memory hierarchy.

Required regions:

- External source buffers.
- External destination buffers.
- External DMA-coherent command rings.
- External DMA-coherent completion rings.
- External DMA-coherent event rings.
- Optional external extended descriptor payloads.
- PDMA private local storage.
- PDMA data-path buffers.

The ring model should include producer index, consumer index, queue depth, wrap or phase handling, full/empty checks, and ownership transitions.

### PDMA Device Model

The PDMA device model should contain hardware-owned components and explicit inter-component interfaces. It should not directly consume high-level SDK objects.

Proposed components:

- `CommandQueueManager`
- `DescriptorFetcher`
- `Scheduler`
- `QueueGroup`
- `DmaReadEngine`
- `ComputePipeline`
- `DmaWriteEngine`
- `CompletionUnit`
- `EventUnit`
- `InterruptController`
- `TraceSink`

## Scheduler Coprocessor Modeling

The scheduler coprocessor should not be modeled as a real CPU or ISA-level simulator in v0. The first model should focus on scheduler behavior, local state, and interfaces to other PDMA components. This keeps the model useful for software/hardware partitioning without committing to whether the final scheduler is a small MCU, a micro-sequencer, or a hardware state machine.

### v0 Modeling Level

The v0 scheduler should be a behavioral scheduler with explicit local state and message-based interfaces.

It should consume:

- Fetched external command descriptors.
- Queue doorbell notifications.
- Engine completion messages.
- Engine error messages.
- Timer or watchdog ticks, if needed for timeout scenarios.

It should produce:

- Internal DMA read work.
- Internal compute work.
- Internal DMA write work.
- Completion records.
- Event records.
- Interrupt requests through the interrupt model.

This can be represented conceptually as:

```text
Scheduler
  owns:
    task_state_table
    dependency_table
    queue_group_state
    resource_credits
    internal_descriptor_queues

  consumes:
    FetchedCommand
    QueueDoorbell
    EngineDone
    EngineError
    Tick

  produces:
    ReadWork
    ComputeWork
    WriteWork
    CompletionRecord
    EventRecord
    InterruptRequest
```

### Internal State To Model

The scheduler state is more important than a processor instruction stream for v0. The model should include at least:

| State | Purpose |
| --- | --- |
| Task state table | Tracks command ID, queue ID, group ID, lifecycle state, dependency count, outstanding internal work, completion target, and error policy. |
| Dependency table | Tracks graph edges, ready lists, fences, barriers, and wait counts. |
| Queue group state | Tracks queues in each group, arbitration policy, priority, credits, and outstanding limits. |
| Resource state | Tracks read slots, write slots, compute slots, internal descriptor capacity, and completion/event ring availability. |
| Abort state | Tracks failed commands, dependent commands to cancel, cleanup progress, and event publication. |

### Behavioral API Shape

The exact Rust API can be decided later, but the conceptual interface should look like a message processor rather than a CPU emulator.

```text
SchedulerInput:
  CommandFetched(command)
  Doorbell(queue_id)
  EngineDone(done)
  EngineError(error)
  Tick

SchedulerOutput:
  IssueRead(read_work)
  IssueCompute(compute_work)
  IssueWrite(write_work)
  PublishCompletion(record)
  PublishEvent(record)
  RaiseInterrupt(reason)
```

This interface allows tests to inject engine completions and errors directly, replace scheduling policy implementations, and observe exactly when internal work is issued.

### Later Refinement Levels

If scheduler programmability becomes a design question, the model can be refined in later versions without changing the whole device model.

Possible later levels:

- Firmware-like scheduler loop: model explicit scheduler actions such as fetch, decode, issue, collect, publish, and abort.
- Microcode-like scheduler: model a small action set or bytecode if the architecture needs a programmable scheduler engine.
- ISA-level scheduler: model a real processor only if firmware binary compatibility, instruction cost, or cycle-level scheduler performance becomes important. A later `TerminusSchedulerBackend` can use the existing `terminus` RISC-V ISA simulator as a scheduler backend, and `terminus_cosim` provides a useful reference for using the simulator as a library in a SystemVerilog co-simulation environment.

The v0 model should deliberately avoid the ISA-level option unless there is a concrete reason to explore it.

## Async Versus Step-Based Execution

There are two plausible execution styles.

### Step-Based Model

The model exposes a deterministic `step()` or `run_until_idle()` method. Each step advances components in a fixed order.

Advantages:

- Very deterministic.
- Easy to unit test.
- Simple to debug.
- No async runtime dependency.

Disadvantages:

- Can obscure natural producer-consumer concurrency.
- Backpressure and scheduling interactions may become artificial.
- Component boundaries can collapse into step ordering details.

### Async Component Model

Each major hardware component is modeled as an async task connected by typed channels. Rings and memory remain explicit shared objects, but engine interactions use async request/response interfaces.

Advantages:

- Natural fit for command fetch, scheduler dispatch, engine completion, and event generation.
- Backpressure can be represented by bounded channels.
- Component interfaces become explicit.
- Easier to later experiment with concurrent execution and arbitration.

Disadvantages:

- More moving parts.
- Requires careful deterministic test control.
- Async scheduling can hide ordering bugs if not instrumented.

### Recommendation For v0

Use a hybrid approach:

- Start with deterministic scenario tests and a top-level `run_until_idle()` API.
- Internally model component interfaces as message passing concepts.
- Keep the implementation open to async tasks for scheduler/engine interactions.
- Do not require real wall-clock timing or parallel threads in v0.

This lets the model document async-friendly interfaces without forcing async complexity into the first prototype.

## External Versus Internal Descriptors

The model should make the descriptor split explicit.

```text
SDK logical task
  -> driver emits external command descriptor
  -> descriptor fetcher reads external descriptor
  -> scheduler builds normalized task record
  -> scheduler emits internal execution descriptors
  -> engines execute internal descriptors
  -> completion/event records return to software
```

External descriptors should be stable ABI-like structs. Internal descriptors should be hardware-private Rust enums or structs.

Example external descriptor categories:

- `Copy`
- `ComputeCopy`
- `Fence`
- `Barrier`
- `GraphLink`

Example internal descriptor categories:

- `ReadChunk`
- `ComputeDispatch`
- `WriteChunk`
- `WaitDependency`
- `PublishCompletion`
- `PublishEvent`

## Multi-Queue Modeling

The model should include multi-queue behavior early because it affects many interface decisions.

Minimum v0 support:

- Memory-resident queue directory.
- Software-provisioned queue configuration entries.
- Multiple command rings.
- One or more completion rings.
- Optional shared completion ring across multiple command rings.
- One device-level event ring.
- Queue IDs and command IDs in completion records.
- Queue groups that bind command queues to engine resources.
- Hardware active queue context cache.
- Weighted round-robin arbitration as the baseline policy.

The model should distinguish provisioned queues from active queues:

- Provisioned queues live in the modeled external memory queue directory and are limited by model memory, queue ID width, and configured directory size.
- Active queues are cached inside the modeled hardware and are limited by active queue context cache capacity and scheduler resources.

The v0 model should not assume one hardware register block per queue. A shared doorbell carrying `queue_id` and producer index is the preferred baseline. A memory-resident doorbell queue can be modeled later if needed.

Doorbell handling should be modeled as a hardware front-end before the scheduler. The driver writes a doorbell record, the doorbell portal captures it, the queue manager coalesces same-queue notifications, and the scheduler later sees a runnable queue. The scheduler should not be modeled as the raw MMIO doorbell endpoint.

Baseline queue ownership semantics:

- Each command queue is single-producer/single-consumer.
- Multiple software clients use different queues or software synchronization above the PDMA model.
- Multiple doorbells for the same queue may still occur over time from the same producer.
- Same-queue doorbells may be coalesced by retaining the latest producer index.
- Software does not read a hardware consumer register on every submit.
- Software ring-space accounting is based on local shadow state updated by completions or events.
- In v0, a command ring entry becomes reusable after the corresponding command completion is observed.

Additional structures to model:

- `QueueDirectory`
- `QueueConfigEntry`
- `ActiveQueueContext`
- `ActiveQueueContextCache`
- `DoorbellPortal`
- `DoorbellRecord`
- `DoorbellPendingTable`

The model should make it easy to compare alternatives:

- One command ring per queue group.
- Multiple command rings per queue group.
- One completion ring per command ring.
- Shared completion ring per group.
- Device-level versus group-level event rings.
- Direct MMIO doorbell versus shared doorbell versus memory-resident doorbell queue.
- Static active queue context cache versus refill/evict behavior.

## Optional Compute Modeling

The compute pipeline should be intentionally simple in v0. Its purpose is to validate the interface, not to define final compute capability.

The v0 data path should contain three choices:

- Bypass path: `read -> write`.
- Transform engine: `read -> transform -> write` for a simple single-source operation.
- Vector-add engine: `read src0 + read src1 -> vector_add -> write dst`.

The transform engine can use a toy operation such as add-constant or XOR-constant. The exact operation is less important than validating the interface for a single-source compute stage.

The vector-add engine is included because it forces the model to represent multi-source reads, stream synchronization, compute input contracts, output backpressure, and a reduction-like dataflow. Even if final hardware does not implement vector add directly, it is a useful stress case for interface design.

The compute model should expose capability discovery so the driver model can reject unsupported compute requests.

## v0 Transfer Operations

v0 should implement a small set of high-level transfer operations. These operations are chosen to test the scheduler-lowering concept without expanding the model into a general tensor/dataflow framework.

### Linear Copy

```text
src -> dst
```

This is the baseline path. It validates queue submission, descriptor fetch, scheduler lowering, DMA read/write dispatch, completion records, and basic performance annotation.

### 2D Strided Copy

```text
for row in 0..rows:
  copy src_base + row * src_stride
    -> dst_base + row * dst_stride
```

This validates shape/stride descriptors and scheduler lowering. The read and write engines still execute simple linear chunks; the scheduler expands the high-level operation into row-level internal work items.

### Broadcast Copy

```text
src -> dst0
src -> dst1
...
src -> dstN
```

This validates fanout. The v0 implementation can model broadcast as repeated read plus multiple writes. The model should keep the interface open for a future one-read fanout optimization if performance analysis shows repeated reads are a bottleneck.

### Vector Add

```text
src0 + src1 -> dst
```

This validates multi-source compute and stream synchronization. The scheduler should lower a vector-add command into two read streams, one compute dispatch, one write stream, and a command-level completion.

### Deferred Advanced Operations

The following operations should be deferred until the v0 mechanisms are stable:

- 3D and high-dimensional tensor-like copy.
- General gather/scatter.
- Tile reorder and transpose.
- Multi-stage reduce trees.
- General programmable compute kernels.

The descriptor model should leave room for shape/stride and extended payloads, but v0 should only implement the four operations above.

## Error And Event Modeling

Errors should be first-class in v0 because event queue design affects software and hardware boundaries.

Initial error classes:

- Invalid descriptor type.
- Invalid queue ID.
- Ring full or ring malformed.
- Unsupported compute operation.
- Source or destination out of modeled memory range.
- Dependency cycle detected by software or scheduler.
- Engine failure injected by test.

Each error should produce an event record with enough identity to debug the failure:

- `queue_id`
- `group_id`
- `command_id`
- `engine_id`, when applicable
- `error_code`
- `recoverability`
- optional address or descriptor index

## Tracing Proposal

Tracing should be part of the model from the beginning. The trace should answer why a command completed, stalled, failed, or reordered.

Useful trace events:

- Register read/write.
- Doorbell write.
- Command descriptor written by driver.
- Command descriptor fetched by hardware.
- Queue arbitration decision.
- Internal descriptor issued.
- Read engine start/done.
- Compute start/done.
- Write engine start/done.
- Completion record written.
- Event record written.
- Interrupt raised.
- Backpressure asserted and released.
- Stall reason entered and exited.
- Resource credit granted or exhausted.
- Queue context cache hit, miss, refill, and eviction.

Chrome tracing JSON is a good first output format because it is easy to inspect and was already useful in `etha_model`. A structured JSON event stream should also be considered so Python or Rust analysis tools can summarize queue occupancy, bytes moved, latency, and stall reasons.

## Performance Exploration Extension

The conceptual model should reserve space for performance exploration through annotations. This is not cycle-accurate modeling. The goal is to identify likely bottlenecks and hotspots, divide system-level performance targets into component-level budgets, and quickly re-evaluate architecture changes when later design stages show that a target is unrealistic.

The model should support two modes of use:

- Given a component performance profile, run scenarios and report bottlenecks.
- Given a system-level target, estimate required component-level budgets.

### Performance Annotation Layer

The functional model should remain the source of architectural truth. Performance annotations should attach abstract cost, capacity, and resource limits to components without changing command semantics.

Example annotated component parameters:

| Component | Example annotations |
| --- | --- |
| Scheduler | command decode rate, issue rate, active queue context cache size, context miss penalty, dependency evaluation cost. |
| Descriptor fetcher | fetch bandwidth, fetch latency, maximum in-flight descriptor fetches. |
| DMA read engine | read bandwidth, setup latency, maximum outstanding reads, burst size, alignment penalty. |
| Compute unit | operation throughput, setup latency, input buffer depth, output buffer depth, supported operation classes. |
| DMA write engine | write bandwidth, setup latency, maximum outstanding writes, reorder depth. |
| Memory system | read bandwidth, write bandwidth, base latency, contention policy. |
| Rings and queues | command depth, completion depth, event depth, doorbell FIFO depth, active queue cache capacity. |

The model can use an abstract time unit. The unit does not need to correspond to a real clock cycle. It only needs to be consistent within one scenario so relative bottlenecks and budget pressure can be compared.

### Stall Reason Taxonomy

The trace and analysis layer should classify stalls explicitly. Initial stall reasons should include:

- `CommandRingEmpty`
- `DoorbellFifoFull`
- `ActiveQueueContextMiss`
- `ActiveQueueContextFull`
- `InternalDescriptorFull`
- `DependencyWait`
- `ArbitrationWait`
- `ReadCreditExhausted`
- `ReadBandwidthLimited`
- `ComputeBusy`
- `ComputeInputFull`
- `ComputeOutputFull`
- `WriteCreditExhausted`
- `WriteBandwidthLimited`
- `CompletionRingFull`
- `EventRingFull`
- `MemoryReadContention`
- `MemoryWriteContention`

Each stall event should carry enough context to attribute the bottleneck:

- component ID
- queue ID, when applicable
- group ID, when applicable
- command ID, when applicable
- stall reason
- abstract start and end time or accumulated duration

### Scenario Report

The model should produce a scenario-level report in addition to raw traces. A useful report should include:

- Total abstract time.
- Effective bytes moved per abstract time unit.
- Per-component utilization.
- Per-queue average wait and maximum wait.
- Queue group utilization and fairness indicators.
- Top stall reasons by accumulated time.
- Ring occupancy high-water marks.
- Active queue context cache hit/miss rate.
- Read/write/compute bandwidth demand versus configured budget.

Example report shape:

```text
scenario: two_queue_compute_copy
total_time: 12000 abstract_units
effective_throughput: 18.2 GB/s-equivalent

top_stalls:
  42% WriteBandwidthLimited
  23% ComputeOutputFull
  18% ActiveQueueContextMiss
  9% CompletionRingFull
  8% ReadEngineIdle

utilization:
  scheduler: 71%
  read_engine0: 63%
  compute0: 91%
  write_engine0: 98%

queue_stats:
  q0_avg_wait: 120 abstract_units
  q1_avg_wait: 430 abstract_units
  q1_starvation_warnings: 2
```

### Budget Exploration

The same annotations should help derive component targets from system constraints. For example, given a target copy throughput, active queue count, command size distribution, and latency goal, the model should make it possible to estimate:

- Minimum scheduler issue rate.
- Minimum descriptor fetch bandwidth.
- Required read and write engine bandwidth.
- Required compute throughput for selected operation mixes.
- Required active queue context cache size to keep miss rate below a threshold.
- Completion/event path rate needed to avoid backpressure.

The first version does not need automatic optimization. It only needs to expose the hooks, counters, and reports needed to run parameter sweeps and compare alternatives quickly.

## Proposed Rust Crate Shape

This is a proposal for discussion, not an implementation commitment.

```text
pdma/model/
  docs/
    conceptual_model_v0.md
    architecture_mapping.md
    performance_annotation.md

  modeling/
    Cargo.toml
    src/
      lib.rs
      ids.rs
      memory.rs
      rings.rs
      desc/
        external.rs
        internal.rs
      sdk.rs
      driver.rs
      device.rs
      scheduler.rs
      queue_group.rs
      engines/
        read.rs
        transform.rs
        vector_add.rs
        write.rs
      completion.rs
      event.rs
      interrupt.rs
      trace.rs
    examples/
      rust/
    python/
      trace_parser.py
```

Potential crates to consider later:

- `serde` for trace and config serialization.
- `thiserror` for typed model errors.
- `bitflags` for descriptor flags.
- `futures` only if runtime-independent async traits become useful.
- `tokio` or `async-std` only if a later version needs a concrete async runtime.

Initial v0 should avoid C FFI, header generation, and concrete register byte layout. Those should be added after queue/config/ring and descriptor semantics stabilize.

## First Scenario Tests

The first model should be driven by scenarios rather than broad APIs.

The v0 application scenarios should be synthetic. Their purpose is to validate mechanisms, not to represent final customer workloads. The recommended scenario set should cover smoke correctness, scheduler lowering, multi-queue behavior, bandwidth pressure, and backpressure propagation.

### Scenario 1: Linear Copy Smoke

```text
SDK builds copy task
driver emits external Copy descriptor
driver rings doorbell
scheduler fetches descriptor
scheduler emits ReadChunk and WriteChunk
read/write engines move data
completion record appears
destination equals source
```

### Scenario 2: 2D Strided Copy Lowering

```text
SDK builds 2D strided copy task
driver emits external Strided2dCopy descriptor with rows, row_bytes, src_stride, and dst_stride
scheduler lowers descriptor into row-level ReadChunk and WriteChunk work
read/write engines execute linear chunks
completion record appears once for the whole command
destination rows equal expected source rows
```

### Scenario 3: Broadcast Copy Fanout

```text
SDK builds broadcast task with one source and multiple destinations
driver emits external BroadcastCopy descriptor
scheduler lowers command into repeated read/write pairs or a modeled fanout sequence
completion record appears once all destinations are written
all destination buffers equal source
performance report can show repeated-read cost
```

### Scenario 4: Transform Engine Copy

```text
SDK builds compute-copy task
driver emits external ComputeCopy descriptor
scheduler checks compute capability
read engine fetches data
transform engine applies a single-source operation such as add-constant or XOR-constant
write engine writes result
completion record appears
destination equals expected transform
```

### Scenario 5: Vector Add

```text
SDK builds vector-add task using src0, src1, and dst
driver emits external VectorAdd descriptor
scheduler checks vector-add capability
read engine fetches src0 and src1 streams
vector-add engine computes dst = src0 + src1
write engine writes result
completion record appears
destination equals expected vector sum
```

### Scenario 6: Two Queues One Completion Ring

```text
queue 0 submits command A
queue 1 submits command B
both queues bind to one completion ring
scheduler arbitrates between queues
completions include queue_id and command_id
software routes completions correctly
```

### Scenario 7: Same-Queue Doorbell Coalescing

```text
queue 0 submits commands 0..7
driver sends doorbell(queue=0, producer=8)
before hardware drains queue 0, driver submits commands 8..15
driver sends doorbell(queue=0, producer=16)
doorbell front-end keeps one pending queue 0 entry with producer=16
scheduler sees queue 0 runnable once and fetches through producer=16
```

### Scenario 8: Submit Without Fast-Path Consumer Polling

```text
driver tracks command ring free space with local shadow state
driver pushes commands and sends doorbell without reading hardware consumer state
driver consumes completion records to reclaim command ring entries
if ring appears full, driver polls completions and retries submission
```

### Scenario 9: Queue Group Resource Binding

```text
queue group 0 owns read/write engine pool 0
queue group 1 owns read/write engine pool 1
commands submitted to each group only use the group's resources
trace shows group-local arbitration and execution
```

### Scenario 10: Error Event

```text
driver submits unsupported compute operation
scheduler rejects or compute dispatch fails
event record is written
interrupt is raised
no successful completion is produced for the failed command
```

### Scenario 11: Dependency

```text
task B depends on task A
scheduler does not issue B until A completes
completion order respects dependency
trace shows dependency wait and release
```

### Scenario 12: Bandwidth-Limited Copy

```text
scenario config sets read or write bandwidth below requested traffic
copy workload runs through bypass path
performance report attributes stalls to ReadBandwidthLimited or WriteBandwidthLimited
effective throughput matches the configured bottleneck direction
```

### Scenario 13: Backpressure Propagation

```text
scenario config limits compute output buffer or completion ring depth
upstream read issue is throttled
trace records backpressure reason and duration
scenario report shows top stall reason and affected queue/group
```

The first implementation does not need to complete all scenarios at once. A practical order is:

1. Linear copy smoke.
2. 2D strided copy lowering.
3. Vector add.
4. Two queues sharing one completion ring.
5. Bandwidth-limited copy.
6. Backpressure propagation.

## Post-v0 Architecture Spec Draft

After the v0 model validates the key mechanisms, stable architecture content should move from white-board form into `pdma/model/docs/pdma_arch_spec_v0.md`. That document should be a spec-like architecture draft covering the externally visible subsystem contract and the internal architecture boundaries that the model has proven useful.

The draft should at least cover:

- Queue directory, queue configuration, command ring, completion ring, and event ring ownership.
- Shared doorbell semantics, coalescing behavior, and authoritative producer state.
- Scheduler coprocessor responsibilities and the boundary between external descriptors and internal execution descriptors.
- Data-path blocks, optional compute bypass/attachment behavior, and selected v0 operations.
- Completion, event, interrupt, error, reset, and debug behavior.
- Performance-annotation model, stall reasons, and scenario-report expectations.

The spec draft does not need to freeze byte-level ABI details immediately, but it should clearly identify which descriptor layouts, register offsets, generated headers, and C ABI bindings remain abstract versus stable.

## Open Discussion Points

- How byte-accurate should the first queue directory and ring layout be?
- Which fields of `QueueConfig`, `CompletionRecord`, and `EventRecord` must be considered stable before moving from white-board notes to `pdma/model/docs`?
- How much of the register model should exist in v0 beyond capability, queue directory base, shared doorbell, status, and interrupt state?
- Should tracing use only Chrome trace JSON, or also a structured event log for analysis?
- Should queue groups be statically configured in test setup, or dynamically configured through register writes?
- Should internal descriptors be visible only through trace, or exposed through debug APIs for tests?
- How close should the SDK-like layer be to a future real SDK?
- What is the minimum interface needed to later plug in `terminus` as a RISC-V scheduler backend?
