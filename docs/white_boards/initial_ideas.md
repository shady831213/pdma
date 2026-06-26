# Programmable DMA Subsystem (PDMA)

## Purpose And Scope

PDMA is a memory-to-memory DMA subsystem framework that targets complex, programmable data movement inside a larger system integration. It is designed as an internal subsystem-level IP deliverable, not as a full SoC or DPU product. The subsystem moves data from source memory to destination memory and can optionally perform in-path computation on the data before it reaches the destination.

The primary purpose is to give software a unified way to describe data movement and optional transformation while giving hardware a modular, extensible path for executing those descriptions. The scope includes the hardware blocks that perform the data movement and scheduling, the descriptor and command formats that define the external ABI, the software driver and SDK layers that prepare and submit work, and the verification, modeling, RTL, and implementation collateral needed to deliver the subsystem as reusable IP.

PDMA deliberately does not define the overall SoC memory map, the external IOMMU isolation policy, or the host CPU architecture. Those are integration concerns outside this subsystem.

## Requirements And Non-Goals

### Requirements

- Provide a memory-to-memory DMA framework for single and multi-transfer operations.
- Support optional in-path computation on the data path, conceptually `src -> optional compute -> dst`.
- Allow the optional compute to be another processor, dedicated hardware compute units, or a combination of both.
- Include a scheduler coprocessor for complex transfer orchestration and task graph execution.
- Maintain baseline compatibility with Linux DMAengine and DPDK DMAdev for generic DMA operations.
- Expose stronger subsystem-specific capabilities through a dedicated SDK.
- Report errors through an event queue plus interrupt mechanism.
- Deliver the subsystem as internal IP, including SDK, subsystem-level verification, modeling, RTL, and SDC/implementation collateral.

### Non-Goals

- PDMA is not a full SoC or DPU product.
- PDMA does not own address-space isolation policy. An external IOMMU is expected to enforce isolation.
- PDMA does not define a fixed set of compute operations beyond the possibility of vector computation. Specific compute operations are integration decisions.
- PDMA does not mandate a single host operating system beyond baseline Linux and DPDK compatibility for generic DMA paths.

## Architecture Overview

PDMA is a programmable DMA framework rather than a fixed-function DMA controller. Software submits logical task graphs through a driver. The driver validates those graphs and writes command descriptors into command rings that the hardware can read. A scheduler coprocessor inside PDMA maintains execution state, expands or translates command descriptors into internal execution descriptors, and dispatches work to DMA read engines, optional compute pipelines, and DMA write engines. Completion and errors are reported back through a completion ring and an event queue.

The data path is conceptually:

```text
source memory
     |
     v
DMA read engine
     |
     v
optional compute pipeline
     |
     v
DMA write engine
     |
     v
destination memory
```

The control path is conceptually:

```text
host software
     |
     v
SDK / API
     |
     v
driver
     |
     v
command rings (external ABI descriptors)
     |
     v
scheduler coprocessor
     |
     +-> DMA read engine
     +-> optional compute pipeline
     +-> DMA write engine
     |
     v
completion ring + event queue
     |
     v
interrupt -> driver -> software
```

This separation keeps the external ABI stable while allowing the internal execution representation to evolve with new compute modules or scheduling policies.

## Software And Hardware Partition

### Software responsibilities

- The SDK builds logical task graphs. It exposes high-level primitives such as copy, gather, scatter, barrier, fence, and compute-enriched transfer. It does not directly access hardware registers.
- The driver validates the task graph against the current hardware capabilities, maps user buffers through the operating system IOMMU interfaces, translates the logical graph into command descriptors, and submits those descriptors to command rings.
- The driver also polls or services interrupts for completions and errors, and routes event queue entries back to the SDK or kernel abstractions.

### Hardware responsibilities

- The scheduler coprocessor reads command descriptors from command rings, maintains the execution state of in-flight tasks, and issues internal execution descriptors to the data path engines.
- The DMA read engine fetches source data from memory and forwards it toward compute or the write engine.
- The optional compute pipeline transforms data in the move path when a task selects a compute context.
- The DMA write engine commits transformed or copied data to destination memory.
- The completion unit writes completion records to the completion ring.
- The event unit writes error and exception records to the event queue and signals interrupts.

### External integration responsibilities

- The host OS or firmware is responsible for configuring an external IOMMU to isolate PDMA traffic. PDMA uses the translated addresses it is given.
- The SoC integration team is responsible for clock, reset, power, and physical memory map connections. PDMA exposes a well-defined register and doorbell interface but does not define the SoC fabric.

## Block Diagrams

### Subsystem context

```text
+------------------------------------------------------------------+
|                           Host software                          |
|  +----------------+  +----------------+  +--------------------+  |
|  |   Linux / DPDK |  | PDMA SDK       |  |      OS / IOMMU    |  |
|  |   generic API  |  | advanced API   |  |      driver        |  |
|  +-------+--------+  +-------+--------+  +----------+---------+  |
|          |                   |                      |            |
|          +-------------------+                      |            |
|                      |                              |            |
|                      v                              v            |
|               PDMA kernel driver          IOMMU translation      |
|                      |                              |            |
+----------------------+------------------------------+------------+
                       |
                       v
+------------------------------------------------------------------+
|                         PDMA subsystem                           |
|  +----------------+  +----------------+  +--------------------+  |
|  |   Command      |  |   Scheduler    |  |   Completion /     |  |
|  |   rings        |  |   coprocessor  |  |   event rings      |  |
|  +----------------+  +----------------+  +--------------------+  |
|                                |                                 |
|          +---------------------+---------------------+           |
|          |                     |                     |           |
|          v                     v                     v           |
|  +----------------+    +----------------+    +----------------+  |
|  | DMA read       |    | Optional       |    | DMA write      |  |
|  | engine         |--->| compute        |--->| engine         |  |
|  +----------------+    | pipeline       |    +----------------+  |
|                        +----------------+                        |
+------------------------------------------------------------------+
```

### Data path

```text
source memory
     |
     | (AXI/NoC read)
     v
+------------------+
| DMA read engine  |
|  prefetch / FIFO |
+------------------+
     |
     | raw or segmented data
     v
+--------------------------+
| optional compute pipeline|
|  (bypass or transform)   |
+--------------------------+
     |
     | transformed or copied data
     v
+------------------+
| DMA write engine |
|  commit / reorder|
+------------------+
     |
     | (AXI/NoC write)
     v
destination memory
```

### Control and event path

```text
command rings in host memory
     |
     v
+------------------------+
| scheduler coprocessor  |
| fetch / decode / issue |
+------------------------+
     |
     +-------------------> DMA read engine
     +-------------------> optional compute pipeline
     +-------------------> DMA write engine
     |
     v
+------------------------+
| completion ring writer |
+------------------------+
     |
     v
completion ring in host memory
     |
     v
driver -> software

error path:

DMA read engine / compute / write engine
     |
     v
+------------------------+
| event queue writer     |
| interrupt generator    |
+------------------------+
     |
     v
event queue in host memory
     |
     v
interrupt -> driver -> software
```

### Optional compute path detail

```text
+-------------------------------------------------------------+
|                  Optional compute pipeline                  |
|  +----------------+  +----------------+  +----------------+ |
|  | compute        |  | compute        |  | compute        | |
|  | scheduler      |->| execution      |->| data           | |
|  | (inside        |  | unit A         |  | buffer / FIFO  | |
|  | scheduler      |  +----------------+  +----------------+ |
|  | coprocessor)   |                                        |
|  +----------------+  +----------------+  +----------------+ |
|                      | compute        |  | compute        | |
|                      | execution      |->| data           | |
|                      | unit B         |  | buffer / FIFO  | |
|                      +----------------+  +----------------+ |
+-------------------------------------------------------------+
```

The compute scheduler is a logical part of the scheduler coprocessor. Compute execution units may be tightly coupled or loosely connected to the data path, depending on the chosen integration.

## External Software Data Structures

The external ABI uses command descriptors. These are the only structures that software is required to understand. They are kept stable so that the Linux DMAengine and DPDK DMAdev mappings, as well as the advanced SDK, can rely on them.

| Structure | Owner | Purpose | Stability |
| --- | --- | --- | --- |
| Device context | Driver / SDK | Represents one PDMA instance, its capabilities, register aperture, queue limits, and feature bits. | Stable SDK object. |
| Queue context | Driver / SDK | Represents a command/completion/event ring set, queue depth, priority, interrupt mode, and polling mode. | Stable SDK object. |
| Memory handle | OS / driver | References memory already validated and mapped by the external IOMMU path. PDMA consumes the resulting DMA addresses or handles but does not own isolation policy. | Integration-defined. |
| Command descriptor | Driver-produced ABI | Describes one copy, compute-assisted copy, barrier, fence, or graph-link operation. | Stable ABI. |
| Compute operation descriptor | SDK / driver | Carries compute context ID, parameter pointer or inline payload, input/output shape, and execution flags. | Versioned ABI extension. |
| Task graph descriptor | SDK / driver | Describes multiple commands and dependency edges for advanced submissions. | Versioned SDK ABI. |
| Completion record | Hardware-produced ABI | Reports successful command completion with command ID, status, bytes completed, and user cookie. | Stable ABI. |
| Event record | Hardware-produced ABI | Reports errors, exceptions, queue warnings, and diagnostic information. | Stable ABI. |

A command descriptor is the top-level unit of work submitted by the driver. It includes:

- A descriptor type field that selects copy, compute-assisted copy, barrier, fence, or task-graph link.
- Source and destination address fields, which may be physical or IOMMU-translated addresses depending on integration.
- Transfer size and stride information for one-dimensional or multi-dimensional movement.
- An optional compute context identifier that selects a compute module and a compute-specific micro-program.
- Dependency information, such as task identifiers or fence tokens, for graph ordering.
- A completion flag and an application-provided completion cookie.
- An error handling hint that tells the scheduler whether to abort or continue on failure.

Command descriptors may be chained or linked through a next-pointer or a separate link descriptor so that multi-transfer task graphs can be expressed without requiring a single large descriptor.

The external ABI also defines ring control structures that the driver writes and the scheduler coprocessor reads:

- Command ring base, size, head pointer, and tail pointer.
- Completion ring base, size, head pointer, and tail pointer.
- Event ring base, size, head pointer, and tail pointer.
- Doorbell notification path for notifying the scheduler that new commands are available.

## Internal Hardware Data Structures

Inside PDMA, command descriptors are translated into one or more execution descriptors. These internal structures are not visible to software and may change as the hardware evolves. They are listed here to show how the scheduler coprocessor partitions work among the engines.

| Structure | Owner | Purpose |
| --- | --- | --- |
| Fetched command record | Scheduler coprocessor | Local copy of an external command descriptor after fetch, bounds check, and decode. |
| Normalized task record | Scheduler coprocessor | Internal representation of command state, dependency count, output event target, and abort policy. |
| Dependency record | Scheduler coprocessor | Tracks graph edges, fences, barriers, and ready/running/completed state transitions. |
| DMA read descriptor | Scheduler to read engine | Describes one source read chunk, burst policy, stride, stream ID, and destination within the internal data path. |
| Compute dispatch record | Scheduler to compute unit | Describes selected compute unit, input stream, output stream, parameter payload, and timeout policy. |
| DMA write descriptor | Scheduler to write engine | Describes one destination write chunk, burst policy, stride, ordering class, and completion dependency. |
| Engine completion record | Engine to scheduler | Reports internal read, compute, or write completion before external completion is generated. |
| Event/error record | Scheduler or engine to event unit | Carries fault source, command ID, internal descriptor ID, address/status detail, and recoverability classification. |

An internal execution descriptor may represent:

- A memory read chunk issued to the DMA read engine.
- A compute task issued to the optional compute pipeline, including input buffer references and output buffer references.
- A memory write chunk issued to the DMA write engine.
- A barrier or fence token that gates later execution descriptors until earlier ones complete.
- A completion record reservation that the completion unit fills when all preceding descriptors for a command finish.

The scheduler coprocessor maintains an internal task state table that tracks which commands are fetched, which dependencies are satisfied, which execution descriptors are in flight, and which completion records are pending. This table may be implemented in tightly coupled memory or a dedicated scratch buffer.

## Memory Layout And Storage Ownership

PDMA uses different storage classes for software-visible queues, internal scheduling state, and data-path buffering. The placement is intentional: software-visible objects live in externally allocated DMA-coherent memory, while hardware-private execution state stays inside the subsystem or in integration-provided local memory.

```text
external memory visible to host software and PDMA
  +--------------------------------------------------------------+
  | source buffers                                                |
  | destination buffers                                           |
  | command rings                                                 |
  | completion rings                                              |
  | event rings                                                   |
  | optional large command payloads / task graph payloads          |
  | optional compute parameter blocks                             |
  +--------------------------------------------------------------+

PDMA register aperture
  +--------------------------------------------------------------+
  | capability registers                                          |
  | queue directory base / format / status registers              |
  | shared doorbell portal                                        |
  | doorbells                                                     |
  | interrupt mask / status                                       |
  | reset / debug / performance counter registers                 |
  +--------------------------------------------------------------+

PDMA private local storage
  +--------------------------------------------------------------+
  | fetched command cache                                         |
  | normalized task table                                         |
  | dependency table                                              |
  | internal DMA read / compute / DMA write descriptors            |
  | engine completion queues                                      |
  | scheduler firmware or microcode storage, if implemented        |
  +--------------------------------------------------------------+

PDMA data-path storage
  +--------------------------------------------------------------+
  | read prefetch buffers                                         |
  | stream FIFOs                                                  |
  | compute input / output buffers                                |
  | write combining / reorder buffers                             |
  +--------------------------------------------------------------+
```

| Object | Typical location | Allocator / owner | Consumer | Notes |
| --- | --- | --- | --- | --- |
| Source buffers | External memory | Application, kernel, or DPDK memory manager | DMA read engine | Must be mapped for PDMA by the external IOMMU path before submission. |
| Destination buffers | External memory | Application, kernel, or DPDK memory manager | DMA write engine | Completion is published only after destination writes are visible in the configured coherency domain. |
| Command rings | External DMA-coherent memory | Driver | Command queue manager and descriptor fetcher | Software-visible ABI. Holds external command descriptors or pointers to extended payloads. |
| Completion rings | External DMA-coherent memory | Driver | Completion unit produces, driver consumes | Used for normal successful command completion. |
| Event rings | External DMA-coherent memory | Driver | Event unit produces, driver consumes | Used for errors, exceptions, and selected asynchronous notifications. |
| Extended command payloads | External DMA-coherent memory or pinned mapped memory | SDK / driver | Scheduler coprocessor | Used when a task graph, compute parameters, or large scatter-gather list does not fit in the base command descriptor. |
| Register aperture | MMIO | Integration team | Driver and hardware | Holds queue configuration, doorbells, interrupts, reset, debug, and counters. It should not hold high-volume descriptors. |
| Fetched command cache | PDMA private local storage | Hardware | Scheduler coprocessor | Temporary cache of external descriptors after fetch and basic checks. |
| Normalized task table | PDMA private local storage | Scheduler coprocessor | Scheduler coprocessor | Tracks task state, dependencies, in-flight work, abort policy, and completion target. |
| Internal execution descriptors | PDMA private local storage | Scheduler coprocessor | DMA read engine, compute units, DMA write engine | Hardware-private format. Not part of the software ABI. |
| Engine-local queues | PDMA private local storage | Engines | Engines and scheduler | Decouples scheduler issue from engine execution and absorbs short stalls. |
| Data-path buffers | PDMA data-path storage | Hardware | Read, compute, and write engines | Carries payload data, not command metadata. Size is a performance and timing tradeoff. |

The recommended baseline is to keep external rings in DMA-coherent memory allocated by the driver. For user-space SDK and DPDK paths, the driver or PMD should still own queue setup and expose only mapped queue handles or doorbell access according to the final protection model. Internal descriptors should not be placed in host-visible memory unless a debug mode explicitly snapshots them.

## Queue And Ring Model

PDMA uses producer-consumer rings in host memory for all external communication.

### Command rings

The driver is the producer. The scheduler coprocessor is the consumer. The driver writes command descriptors into the ring and updates the tail pointer. It then rings a doorbell. The scheduler fetches descriptors up to the tail pointer and updates its own head pointer. Multiple command rings may exist to support independent streams or priority classes, but each ring is a single producer and single consumer channel.

### Completion rings

The completion unit inside PDMA is the producer. The driver is the consumer. Each completion record contains a command identifier, a status code, and the application cookie from the original command descriptor. The driver reads the ring and updates its head pointer. PDMA stops writing to a completion ring when it is full and may signal an event rather than silently dropping completions.

### Event rings

The event unit inside PDMA is the producer. The driver is the consumer. Event records carry error codes, error addresses, offending descriptor identifiers, and optional diagnostic data. An event ring entry may also carry non-error notifications such as queue-full warnings or scheduler state changes, although the initial design focuses on error reporting.

### Interrupt moderation

PDMA can generate an interrupt when a completion ring crosses a configured threshold, when an event ring entry is written, or when a command reaches its deadline. The exact moderation policy, including coalescing timers and per-ring thresholds, is a deferred integration decision.

## Multi-Queue Model

PDMA should support multiple software-visible command queues. The baseline model borrows the scalable queue shape from NVMe-style submission/completion queues, virtio/RDMA-style memory-backed queues, and the resource-binding idea from accelerator work-queue groups. A queue is a software submission context; a queue group is a hardware scheduling and resource allocation domain.

The preferred direction is memory-resident queue configuration. Software can provision many virtual queues by placing queue configuration entries and rings in external DMA-coherent memory. Hardware does not need a fixed register bank or SRAM context for every provisioned queue. Instead, hardware caches and schedules a bounded number of active queue contexts.

This creates two different limits:

- Provisioned queues: primarily limited by system memory, queue ID width, and ABI-defined queue directory size.
- Active queues: limited by hardware queue context cache, scheduler state, outstanding command capacity, and engine resources.

This is closer to NVMe/RDMA/virtio/GPU queue models than to device-resident work queues such as Intel DSA WQs.

The primary reference pattern is RDMA/NIC-style memory-backed queue contexts with a hardware doorbell front-end and active context cache. NVMe provides a useful simpler reference for submission/completion queue identity and completion records. Virtio provides the useful rule that notification is a hint and memory-resident queue state is authoritative. DSA is mainly a contrast point for device-resident work queues and submission portals.

### Queue directory

The queue directory is a memory-resident table that maps `queue_id` to queue configuration. It is allocated and initialized by software, then exposed to PDMA through a directory base address and format version.

```text
queue directory in external DMA-coherent memory

queue_id 0 -> QueueConfig 0
queue_id 1 -> QueueConfig 1
queue_id 2 -> QueueConfig 2
...
```

A queue configuration entry should describe:

- Queue enable state and version.
- Queue group ID.
- Command ring base, depth, entry size, and producer/consumer state.
- Completion ring binding.
- Event ring binding.
- Priority and ordering mode.
- Maximum outstanding commands or bytes for this queue.
- Optional doorbell state or pending state.

The hardware may fetch a queue configuration entry when a queue is first activated, when a doorbell references a queue not present in the hardware context cache, or when software requests a queue configuration refresh.

### Active queue context cache

Hardware should maintain a bounded active queue context cache. A cached queue context is a hardware-private copy of the memory-resident queue configuration plus transient scheduler state.

```text
memory-resident QueueConfig
  -> descriptor fetcher / queue manager
  -> active queue context cache
  -> scheduler arbitration
```

The cache allows the architectural number of provisioned queues to scale with memory while keeping hardware resources bounded. Cache replacement policy is a hardware design choice, but the model should expose cache miss, refill, and eviction events because they affect latency and scheduling behavior.

### Queue set

A queue set is the software-visible unit allocated by the driver or SDK. In the memory-resident model, a queue set is represented by one queue directory entry plus its associated rings. It contains:

- One command ring.
- One completion ring binding.
- One event ring binding, usually shared at device or group scope.
- One doorbell notification path, which may be a shared doorbell carrying `queue_id` rather than a per-queue MMIO register.
- Queue identity fields such as `queue_id`, `group_id`, priority, ordering mode, and maximum outstanding commands.

Each command queue should be single-producer/single-consumer in the baseline architecture: one software owner produces command entries, and PDMA consumes them. Different software clients should use different command queues or a higher-level software synchronization layer.

The command ring and completion ring do not have to be one-to-one. PDMA should allow both simple and shared completion layouts:

```text
simple layout:

command ring 0 -> completion ring 0
command ring 1 -> completion ring 1

shared completion layout:

command ring 0 ----+
command ring 1 ----+-> completion ring A
command ring 2 ----+
```

When multiple command rings share a completion ring, each completion record must carry enough identity to route the completion back to the submitter:

- `queue_id`
- `command_id`
- `user_cookie`
- `status`
- `bytes_completed`
- phase or wrap bit

This keeps completion processing correct even when commands complete out of order.

### Queue group

A queue group binds one or more command queues to a scheduler domain and a pool of execution resources. It is the right place to express hardware-side quality-of-service and resource allocation policy.

```text
queue group 0
  command ring 0
  command ring 1
  completion ring A
  DMA read engine pool 0
  DMA write engine pool 0
  optional compute units 0..N

queue group 1
  command ring 2
  completion ring B
  DMA read engine pool 1
  DMA write engine pool 1
  optional compute units M..K
```

The initial architecture should allow a command queue to belong to exactly one queue group. An engine or compute unit may either be dedicated to one group or shared across groups through a higher-level arbiter. The first implementation can choose a simpler static binding and leave dynamic rebinding as a later feature.

### Arbitration

The scheduler coprocessor arbitrates command fetch across active queues in the same group. Candidate policies include round-robin, weighted round-robin, strict priority, deficit round-robin, and credit-based scheduling.

The recommended baseline is:

- Weighted round-robin across normal command queues in a group.
- Strict priority for control, urgent, or error-recovery queues.
- Per-queue maximum outstanding command count.
- Per-group maximum outstanding bytes or descriptors.
- Optional compute-resource credits so compute-heavy queues do not block pure-copy queues indefinitely.

Arbitration should select which command descriptors are fetched and decoded. It should not require the data path to understand software queue policy. Once work is translated into internal execution descriptors, the engines see only ready work items and backpressure signals.

### Doorbell and activation

If queue count is memory-scaled, PDMA should avoid one MMIO doorbell register per queue. The baseline notification model should be a shared doorbell mechanism that carries at least `queue_id` and the new producer index, or a memory-resident doorbell queue consumed by hardware.

The doorbell front-end should be a dedicated hardware block, not the scheduler coprocessor itself. Software writes a shared doorbell portal. The host interface decodes the write, creates a doorbell record, and feeds a doorbell FIFO, coalescer, or pending table. The queue manager consumes these records and presents runnable queues to the scheduler coprocessor.

```text
software writes command descriptors
software updates QueueConfig producer index or ring producer index
software sends doorbell(queue_id, producer_idx)
hardware doorbell portal captures the notification
hardware coalesces or records the pending queue
hardware fetches QueueConfig on cache miss
hardware marks queue active
scheduler arbitrates active queues
```

This keeps the number of provisioned queues from being limited by register aperture size. The number of simultaneously active queues is still bounded by the active queue context cache and scheduler resources.

Doorbell is a notification, not ownership transfer and not the only source of truth. The producer index stored in the memory-resident queue state remains authoritative. Multiple doorbells for the same queue can occur over time from the same producer if software appends more commands before hardware drains the queue. The doorbell front-end may coalesce same-queue notifications by keeping the latest producer index and avoiding duplicate active-queue-list entries.

```text
doorbell(q0, producer_idx=8)
  -> mark q0 pending
  -> cached producer for q0 = 8
  -> insert q0 into active queue set

doorbell(q0, producer_idx=16)
  -> q0 already pending
  -> cached producer for q0 = 16
  -> do not insert q0 again
```

The software fast path should not read a hardware consumer register for every submission. Software pushes command entries, updates its producer state, and sends a doorbell. Ring-space accounting should use software shadow state updated by completions or events. If the command ring appears full, software may poll or process completion records to reclaim entries and then retry.

The conservative v0 reclaim rule is: a command ring entry is reusable by software after the corresponding command completion is observed. Later designs may expose an earlier descriptor-consumed index, similar to NVMe SQ head reporting, if early command ring reclamation is needed.

### Event ring mapping

Completion rings are optimized for high-volume normal completions. Event rings are optimized for lower-volume errors and asynchronous notifications. The recommended baseline is:

- One device-level event ring for fatal and global errors.
- Optional per-group event rings for recoverable group-local errors.
- No per-command event ring unless a future use case requires very high event volume.

An event record should include `queue_id`, `group_id`, `command_id`, `engine_id`, `error_code`, `fault_address`, and recoverability classification when those fields are meaningful.

### Software mapping

The generic Linux and DPDK paths should not expose every PDMA queue feature. They should map cleanly onto generic DMA semantics:

- Linux DMAengine can map one PDMA queue or queue group to one DMA channel provider context.
- DPDK DMAdev can map a hardware command queue or queue group to one `dmadev`, with `vchan` used as a software transfer context rather than a guarantee of independent hardware execution.
- The advanced SDK should expose queue set allocation, completion ring sharing, queue group selection, priority, ordering mode, and graph submission.
- The advanced SDK should expose memory-backed queue provisioning and should distinguish provisioned queues from active queues.

This keeps baseline compatibility simple while allowing the SDK to use the full multi-queue model.

## Control Path And Data Path Interaction

The control path never carries payload data. It carries commands, scheduling decisions, descriptor state, completion records, and error records. The data path carries payload data from source memory through optional compute to destination memory. The scheduler coprocessor is the bridge between the two paths.

```text
control path:

driver
  -> command ring
  -> doorbell
  -> command queue manager
  -> descriptor fetcher
  -> scheduler coprocessor
  -> internal execution descriptors
  -> engine-local command queues

data path:

source memory
  -> DMA read engine
  -> read buffer / stream FIFO
  -> optional compute unit or bypass path
  -> write buffer / reorder buffer
  -> DMA write engine
  -> destination memory

completion/error return path:

engines
  -> engine completion/status
  -> scheduler coprocessor
  -> completion unit or event unit
  -> completion/event ring
  -> interrupt or polling
  -> driver / SDK
```

### Submission sequence

1. The SDK or generic DMA API asks the driver to submit work.
2. The driver validates buffers, uses the external IOMMU path to obtain PDMA-visible addresses, and writes external command descriptors into a command ring.
3. The driver updates the command ring producer index and writes a doorbell register.
4. The command queue manager observes the doorbell and makes the queue eligible for descriptor fetch.
5. The descriptor fetcher reads command descriptors into PDMA private local storage.
6. The scheduler coprocessor decodes commands, allocates normalized task records, resolves dependencies, and creates internal execution descriptors.

### Dispatch sequence

1. For a pure copy, the scheduler issues a DMA read descriptor and a matching DMA write descriptor connected by a bypass stream.
2. For compute-assisted work, the scheduler issues a DMA read descriptor, a compute dispatch record, and a DMA write descriptor connected by stream IDs or internal buffer IDs.
3. The read engine fetches source data and pushes it into a data-path buffer or stream FIFO.
4. The compute unit either transforms the stream or the bypass path forwards it unchanged.
5. The write engine drains the output stream and writes destination memory.

### Completion sequence

1. Each engine reports internal completion or error to the scheduler.
2. The scheduler updates the normalized task record and dependency table.
3. When all internal descriptors for a command are done and destination writes are visible, the completion unit writes a completion record.
4. If an error occurs, the event unit writes an event record and the interrupt controller raises an interrupt according to the event policy.
5. The driver consumes completion and event records, updates software state, and notifies the SDK or generic DMA client.

### Backpressure and flow control

Backpressure can originate from command rings, internal descriptor storage, data-path buffers, compute units, write response queues, completion rings, or event rings. The scheduler should treat each backpressure source differently:

- Command ring empty: stop fetching from that queue until a new doorbell or producer update arrives.
- Internal descriptor storage full: stop decoding new commands but allow in-flight engines to drain.
- Data-path buffer full: stop issuing new read descriptors or throttle read burst length.
- Compute unit busy: keep eligible work queued or route pure copy work through the bypass path.
- Write path blocked: throttle read issue to avoid unbounded buffering.
- Completion ring full: stop retiring commands that require completion records and raise an event or status bit.
- Event ring full: raise a fatal or sticky status because error visibility may be compromised.

This split keeps the data path simple and stream-oriented while allowing the control path to absorb complex graph scheduling, dependency handling, and recovery policy.

## Ordering, Completion, And Error Semantics

### Ordering

Ordering is explicit in the command and event model. The default expectation for a single command ring is that commands complete in submission order, but software can request weaker ordering through flags if the hardware supports it.

Independent command rings are not ordered with respect to each other unless software inserts a cross-ring fence or barrier command. Commands within a task graph are ordered by the dependencies declared in their descriptors.

Write visibility is guaranteed before a completion record is written. That is, when software sees a completion record, all destination memory writes for that command are observable to the host according to the SoC memory model.

### Completion

A command is complete when all execution descriptors generated from it have finished without error and all destination writes are visible. At that point, the completion unit writes a completion record to the completion ring. The completion record includes the command identifier, status, and application cookie.

Barriers and fences complete when all preceding descriptors in their dependency scope have finished. Their completion records act as synchronization points for software.

### Error semantics

When an engine detects an error, it reports the error to the event unit. The event unit writes an event record to the event ring and raises an interrupt. Examples of errors include:

- Source or destination address out of allowed range or not aligned as required.
- Command descriptor decode failure.
- Compute execution unit failure or timeout.
- AXI/NoC error response on a read or write.
- Command timeout or deadline violation.

On error, the scheduler coprocessor either aborts the failing command and all dependent commands or marks the command failed and continues with independent work, depending on the error handling hint in the descriptor. The exact abort and cleanup behavior is a deferred decision.

## Task Graph And Scheduler Model

The SDK builds logical task graphs. A task graph is a directed acyclic graph of commands where edges represent data or ordering dependencies. The driver validates the graph, checks that all referenced compute contexts and resources exist, and serializes the graph into command descriptors that respect the declared dependencies.

The scheduler coprocessor maintains execution state for each in-flight task graph. It fetches command descriptors, resolves dependencies, expands commands into internal execution descriptors, and issues those descriptors to the data path engines. The scheduler can interleave execution descriptors from different commands or different task graphs as long as declared dependencies are honored.

Key scheduler functions:

- Fetch and decode command descriptors from one or more command rings.
- Build and update an internal dependency graph for active commands.
- Issue execution descriptors to the DMA read engine, optional compute pipeline, and DMA write engine.
- Track completions from each engine and update dependency state.
- Write completion records and event records.
- Handle command abort and cleanup on error.

The scheduler coprocessor can be a small programmable processor, a micro-sequencer, or a state machine with local program memory, depending on the complexity target. The architecture is agnostic to the exact implementation as long as it performs the functions above.

## Optional Compute Pipeline Model

PDMA supports an optional compute pipeline between the read and write engines. The compute pipeline is not required for every transfer. When it is bypassed, PDMA behaves as a conventional memory-to-memory DMA controller. When it is enabled, data from the read engine is steered into a compute execution unit, transformed, and then forwarded to the write engine.

The v0 conceptual model should implement a small set of transfer operations to test scheduler lowering and composability:

- Linear copy: `src -> dst`.
- 2D strided copy: high-level shape/stride command lowered into row-level internal chunks.
- Broadcast copy: one source copied to multiple destinations, initially modeled as repeated read/write work with room for future one-read fanout optimization.
- Vector add: `src0 + src1 -> dst`, used to validate multi-source reads, compute input synchronization, and compute output backpressure.

More complex operations such as 3D/high-dimensional transfers, general gather/scatter, tile reorder, transpose, and multi-stage reduce trees should be deferred until the v0 mechanisms are stable.

The compute pipeline is intentionally abstract. It may be implemented as:

- A general-purpose processor core running compute microcode.
- A dedicated hardware compute unit, such as a vector execution engine.
- A combination of both, where the scheduler selects the appropriate unit for each task.

The scheduler coprocessor contains a compute scheduler that knows how to dispatch compute tasks to the available compute execution units. The interface between the compute scheduler and the compute execution units is an internal PDMA interface. The exact compute operations are integration decisions, but vector computation is one possible operation class.

Compute descriptors in the command path carry a compute context identifier and a compute-specific payload. The scheduler uses the context identifier to select a compute execution unit and to translate the payload into an internal compute task descriptor.

## Key Hardware Components

The following components define the subsystem boundary. Implementations may merge simple blocks, but the responsibilities should remain architecturally visible for verification and modeling.

### Host interface and register block

The host interface exposes configuration, status, capability discovery, queue directory base registers, shared doorbell portal, interrupt masks, reset controls, and debug state. It is the only register-level interface expected to be programmed directly by the driver.

### Command queue manager

The command queue manager consumes doorbell notifications, fetches memory-resident queue configuration entries, maintains the active queue context cache, tracks producer/consumer state for active queues, and presents runnable queues to the scheduler coprocessor.

### Descriptor fetcher

The descriptor fetcher reads external command descriptors from host memory, performs basic alignment and length checks, and delivers fetched descriptors to the scheduler coprocessor. Complex semantic validation remains a scheduler or firmware function.

### Scheduler coprocessor

The scheduler coprocessor is the central control block. It fetches command descriptors, maintains execution state, and dispatches work. It owns the internal task state table and the compute scheduler.

### DMA read engine

The DMA read engine fetches data from source memory. It supports burst reads, prefetching, and source stride handling. It forwards data either directly to the write engine or to the optional compute pipeline.

### Optional compute pipeline

The optional compute pipeline contains one or more compute execution units and the data buffers that connect them to the read and write engines. The compute scheduler inside the scheduler coprocessor controls which unit executes each compute task.

### DMA write engine

The DMA write engine commits data to destination memory. It supports burst writes, write reordering within the bounds of ordering guarantees, and destination stride handling.

### Completion unit

The completion unit writes completion records to the completion ring in host memory. It ensures that a completion record is written only after all preceding writes for the associated command are visible.

### Event unit

The event unit collects errors and exceptions from all engines, formats event records, writes them to the event ring, and generates interrupts. It may also support interrupt moderation and masking.

### Interrupt controller

The interrupt controller turns completion thresholds, event records, fatal errors, and optional watchdog conditions into host-visible interrupts. It should support masking, status readback, and interrupt moderation.

### Performance counters

Performance counters expose queue occupancy, descriptor fetch latency, read/write bandwidth, compute utilization, completion latency, backpressure cycles, and error counts. These counters are needed for modeling correlation, software tuning, and post-silicon debug.

### Reset and debug block

The reset and debug block provides controlled reset of queues, scheduler state, DMA engines, compute units, and event generation. It should expose enough state to diagnose stuck descriptors without requiring invasive internal visibility.

### Internal interconnect and buffers

The internal interconnect and buffers connect DMA read, compute, and DMA write blocks. They provide flow control, buffering, backpressure propagation, and optional stream tagging so that independent commands can share the pipeline safely.

## SDK And Compatibility Deliverables

### Baseline compatibility

PDMA provides a driver mapping for Linux DMAengine and DPDK DMAdev. These mappings cover generic memory-to-memory copy and scatter-gather operations. Compute-assisted transfers, task graphs, and other advanced features are not exposed through these generic APIs.

### Advanced SDK

The PDMA SDK exposes the full subsystem capabilities. It provides:

- APIs for building logical task graphs.
- APIs for selecting compute contexts and attaching compute payloads.
- APIs for configuring fences, barriers, and dependency edges.
- APIs for waiting on completions and reading event queue entries.
- Headers and tools that define the external ABI command descriptor formats.

The SDK is the preferred interface for applications that need more than a simple copy.

## Verification, Modeling, RTL, And Implementation Collateral

The final deliverable is an internal subsystem-level IP package. The expected collateral includes:

- Functional model plus performance annotation support for the scheduler coprocessor, DMA engines, optional compute pipeline, completion unit, and event unit. The performance model is not cycle-accurate; it is intended to identify likely bottlenecks, hotspots, and component-level budget pressure under system-level constraints.
- Subsystem-level verification environment, including directed tests, randomized tests, and a reference model for comparing hardware behavior against the architecture specification.
- RTL implementation of all key hardware components.
- Synthesis constraints and implementation collateral, including SDC files, floorplan guidance, and timing budgets.
- Software driver and SDK source, including Linux DMAengine and DPDK DMAdev compatibility layers.
- Architecture and integration documentation, including register descriptions, ring layouts, and command descriptor formats.

After the v0 conceptual model validates the core queue, scheduler, doorbell, data-path, completion, and performance-annotation mechanisms, the project should produce a spec-like architecture draft under `pdma/model/docs/pdma_arch_spec_v0.md`. That document should be closer to a real architecture specification than these white-board notes, while still allowing details such as exact descriptor byte layouts, register offsets, generated headers, and C ABI bindings to mature with the model.

The modeling level that is built first will be driven by the verification and software schedule. Candidates include a functional model for driver bring-up, a performance-annotated model for bottleneck and budget exploration, and a verification reference model for scoreboarding.

## Deferred Decisions

The following items are intentionally left open for later decision:

- Which compute operations are supported first beyond the possibility of vector computation.
- Whether the first implementation uses a programmable compute processor, fixed-function hardware compute units, or a minimal compute interface that can support both later.
- Exact ordering guarantees for single-queue, multi-queue, and compute-assisted transfers, including whether weak ordering modes are exposed.
- Number of command queues, completion queues, queue groups, and event rings supported by the first implementation.
- Whether engine and compute resources are statically bound to queue groups or dynamically shared across groups.
- Exact queue arbitration policy, priority levels, credit model, and starvation-prevention rules.
- Exact event formats, error codes, and interrupt moderation policies.
- The first modeling level to build: functional model, performance model, driver model, or verification reference model.
- The exact implementation of the scheduler coprocessor as a processor, micro-sequencer, or state machine.
- Command abort and cleanup behavior on error, including how dependent commands are notified and released.
