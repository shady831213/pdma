# PDMA functional model

The reusable libraries and their extraction boundaries are documented in
[reusable libraries](docs/reusable-libraries.md).

The [system specification](docs/spec.md) includes hardware and firmware requirements,
Host/platform obligations, and explicit model-to-RTL verification boundaries.
[Managed fatal/events](docs/fatal-events.md) specifies the PF event ring and explicit
DMA/event recovery. Raw DMA-wide fatal notification goes to firmware; PF receives
firmware events and coordinates affected VFs.

The DMA command slot is a naturally aligned 96-byte overlay of complete commands.
All commands reuse one `CommandHeader`/opcode definition. Generic untagged union
storage never interprets an opcode; PDMA contract decoding selects the command,
while Copy's endpoint `tagged_union` fields retain their local mode tags.
Complete command definitions live together in the contract's operations module;
the command module owns the top-level union, shared header and protocol codec.
Schema's optional `codec: external` generates convenience methods and trait/view
adapters that call the application's `SchemaCodec` implementation. Command owns
that implementation, including opcode interpretation and protocol checks. See the
[wire layout](docs/copy-engine.md#wire-layout) and [layout decisions](docs/layout-review.md).

Composition uses `SchemaLayout` for each child's value, extent, alignment and
views, and `SchemaCodec: SchemaLayout` for encoding/decoding. Generated records
implement both automatically; generated untagged unions provide their layout and
need only application encode/decode methods for complete codec composition.
Handwritten types can implement both without record-specific inherent methods
or structural metadata. Each child owns its internal validation, while a union
checks its own inactive tail. An inline tagged union still receives its tag from
the enclosing record's codec.

The [roadmap and SDMA gap comparison](docs/firmware-dma-goal.md#copy-engine-synchronization-reassessment)
record the withdrawal of the first Signal/Wait implementation. The approved
[copy-engine contract](docs/copy-engine.md) now defines Copy, Write, Fill, Wait,
Atomic, Semaphore, Fence, Trap and Timestamp. Success produces no event; Fence stores completion
and Trap explicitly requests notification. Generic memory atomics and reset during
context maintenance remain available. HSA is a semantic reference rather than an
AQL compatibility target. The broader
firmware-managed direction preserves the ordinary configuration and prepares
SR-IOV, PASID and Function reset integration boundaries.
The implemented first milestone is the [architecture MVP](docs/firmware-dma-mvp.md): an
independent firmware-managed assembly that runs a real copy while retaining the
direct-DMA scenarios. Its [control channel](docs/control-channel.md) now carries
Host acquisition/release requests over separate rings; firmware Ready alone leaves
the DMA work window closed.
The [Function-isolation slice](docs/function-isolation.md) uses one PF management
channel with explicit target Functions, firmware-owned allocation, MMIO permissions and interrupt destinations
for multiple Functions sharing that same DMA. The managed platform can inject
independent memory capabilities per Function, including identical DMA addresses;
ordinary shared-memory assembly remains available. This does not implement PCIe
or IOMMU translation.

The optional [context model](docs/context-contract.md) adds independent ring, SG,
source and destination selectors, with firmware-managed Default/PASID bindings.
Hardware snapshots published producers for Drain/Abort and retains accepted work
until retirement. The document records software obligations for the eventual RTL
specification; model-local memory completion does not prove external bus drain.

The supported toolchain baseline is Rust 1.96 or later (edition 2024), matching
the compiler used to validate the current implementation. This support policy
does not claim that every crate technically requires Rust 1.96 features.
Workspace and standalone fixture manifests declare the same minimum; Clippy uses
that minimum when proposing API replacements. Local toolchain upgrades do not
automatically raise the declared minimum. Strict generated-header tests also
require a C11 compiler (`cc`).

Run from this directory:

```sh
cargo test -p pdma-model-scenarios --test single_channel -- --nocapture
cargo test -p pdma-model-scenarios --test multi_channel -- --nocapture
cargo test -p pdma-model-scenarios --test firmware_managed -- --nocapture
```

The system tests run all four linear/SG endpoint combinations, unequal segment
boundaries, multi-chunk and zero-total transfers, plus event-ring write faults. They use
real memory access, the same composed device pipeline, a software environment, and the
existing interrupt dispatcher. Worker construction/join belongs to the harness;
case modules own inputs and acceptance assertions. Buffer allocation, payload initialization,
software setup, submission, handler registration
and completion waiting run inside one
`InterruptRuntime::run_main` per case, which uses `SoftwareEnv` internally. A channel is transferred once into its persistent
IRQ handler, rather than repeatedly moved through shared optional state.

Runtime-owned memory installs regions through one atomic `add_regions(&regions)` call.
Overlap and capacity checks, backing allocation and installation run under one exclusive
storage access; an error leaves existing regions and bytes unchanged. There are no public
prepared tokens, transaction drafts or memory revisions. The software allocator advances
its address cursor only after installation succeeds. Hardware still receives only
byte-access capabilities, without memory ownership or allocation authority.

Memory backends select their own `MemoryAccess::Error`. Runtime adapters preserve
that type; DMA alone classifies it through `MemoryFailure` for hardware handling.
Routing failures belong to hw-components, and software retains the original
backend errors. New platform error categories need no generic memory-library change.

## Read the implementation

| Location | Responsibility |
| --- | --- |
| `crates/contract/src/dma.rs` | Public DMA contract facade; explicit exports from the owning modules. |
| `crates/contract/src/dma/command.rs` | Tagged command descriptors and Copy payload. |
| `crates/contract/src/dma/operations.rs` | Scalar, Fill, Wait, Semaphore and Timestamp payloads; atomic operations in its child module. |
| `crates/contract/src/dma/endpoint.rs` | Payload endpoint geometry, SG segments and typed positions. |
| `crates/contract/src/dma/event.rs` | Error-only event records and committed progress. |
| `crates/contract/src/register.rs` | Register types shared by DMA and management hardware. |
| `crates/contract/src/dma/registers.rs` | DMA discovery and administration schemas, composed into the top-level register map. |
| `crates/contract/src/dma/registers/channel.rs` | Channel control/IRQ schemas and command/event ring register maps. |
| `crates/contract/src/dma/access.rs` | Channel/context selectors and outgoing DMA sideband; channel address helpers use the register layout. |
| `crates/contract/src/hw_components/windows.rs` | Peripheral channel ownership, access-window control and Host readiness maps. |
| `crates/contract/src/hw_components/context.rs` | Peripheral context table and firmware maintenance registers. |
| `crates/contract/src/hw_components/control.rs` | Hardware bootstrap, activation and bidirectional doorbells. |
| `crates/contract/src/hw_components/firmware.rs` | Hardware registers carrying firmware-published boot status. |
| `crates/contract/src/control.rs` | Host-to-firmware software control messages and memory-resident queue descriptors. |
| `crates/contract/src/memory.rs` | Function identity, extensible address-space attributes and resolved downstream memory identity. |
| `crates/hw-components/src/context.rs` | Context bindings, maintenance result and failure bitmap, with its own memory adapter and construction errors. |
| `crates/hw-components/src/control.rs` | Bootstrap ownership, activation lock and notification pulses, without queue memory or allocation policy. |
| `crates/host/src/management.rs` | Host control-ring setup, channel requests and cancellation-safe response collection. |
| `crates/hw-components/src/windows/window.rs` | One channel's owner/open registers, requester authorization, readiness and complete-transaction bounds. |
| `crates/hw-components/src/windows.rs` | Instance layout, discovery/global routing and selection of channel windows. |
| `crates/hw-components/src/firmware.rs` | Firmware boot report, writable only through management access and readable by Host even with DMA closed. |
| `crates/host/src/firmware.rs` | Host boot polling with distinct firmware failure and software timeout outcomes. |
| `crates/firmware/src/lib.rs` | Async boot and serial control service; resource sequencing in its native child modules. |
| `crates/scenarios/tests/firmware_managed/harness/runtime.rs` | Managed-system assembly with native submodules for concrete transports and scoped actor execution. |
| `crates/hw-model-irq/src/wire.rs` | Shared coalesced notification source with separate emission and claim capabilities. |
| `crates/contract/src/ring.rs` | PDMA ring geometry, shared pointer rules and errors. |
| `crates/software/src/ring.rs` | Shared PDMA software ring configuration and operations over borrowed MMIO or memory cursors. |
| `crates/hw-model-swenv/src/notifications.rs` | Coalesced progress hints independent of interrupt dispatch. |
| `crates/contract/src/bin/pdma-abi-header.rs` | Current C11 header generation through schema exporters. |
| `crates/dma/src/channel.rs` | Channel registers, rings, IRQ and private accepted-work state. |
| `crates/dma/src/channel/mmio.rs` | Generated block routing and software-write effects. |
| `crates/dma/src/stages.rs` | One connected command admission and execution pipeline. |
| `crates/dma/src/stages/fetch.rs` | Revalidated command acceptance into channel-owned state. |
| `crates/hw-model-pipeline/src/combinators/` | Typed sequencing, gathering, arbitration and distribution. |
| `crates/dma/src/arbitration.rs` | PDMA round-robin selection policy. |
| `crates/dma/src/stages/candidate.rs` | Non-consuming channel candidate advancement against the full DMA context. |
| `crates/dma/src/channel/work.rs` | Local operations on retained channel work. |
| `crates/dma/src/channel/work/` | Command traversal and cursor calculations. |
| `crates/dma/src/flow.rs` | Owned Flow and typed command, chunk, payload and completion values. |
| `crates/dma/src/execution_context.rs` | DMA resource bank and input-selected borrowed views with lifecycle guards. |
| `crates/dma/src/work.rs` | Work identity, residency release and invalid-grant handling. |
| `crates/dma/src/stages/execution/validate.rs` | Range validation and validated traversal installation. |
| `crates/dma/src/stages/execution/chunks.rs` | Shared bounded-chunk preparation stage. |
| `crates/dma/src/stages/execution/copy.rs` | Shared chunk read/write stages with selected execution resources. |
| `crates/dma/src/stages/opcodes.rs` | One opcode Dispatch connecting Fetch to the selected command execution branch. |
| `crates/dma/src/stages/operations.rs` | Bounded scalar execution and retained Fill progress; word/atomic access in child modules. |
| `crates/dma/src/channel.rs` | Generation-bound owner-local error reporting and IRQ/fatal state. |
| `crates/software/src/dma.rs` | Shared DMA driver configuration, submit and event consumption; reexported by Host. |
| `crates/host/src/runtime.rs` | PDMA transport: async context access, cooperative yielding and pending notification delivery. |
| `crates/software/src/dma/control.rs` | Shared discovery, fatal, IRQ and register-only reset access. |
| `crates/dma/src/reset.rs` | Reset completion after affected channel-owned work retires. |
| `crates/hw-model-swenv/src/interrupt/runtime.rs` | Environment-bound software execution, ISR registration and dispatcher lifetime. |
| `crates/scenarios/tests/single_channel/main.rs` | Test suite entry and module declarations. |
| `crates/scenarios/tests/single_channel/cases/` | Linear, scatter/gather and fault cases, with acceptance assertions. |
| `crates/scenarios/tests/single_channel/harness/` | Platform provisioning, fault injection and actor driving/cleanup. |
| `crates/scenarios/tests/single_channel/hardware.rs` | Hardware assembly from the existing device and stage graph. |
| `crates/scenarios/tests/single_channel/software/` | Complete software program, buffer allocation and interrupt handler. |
| `crates/scenarios/tests/multi_channel/` | Three-channel reset isolation through the same hardware/software/harness separation. |

`device.hardware()` returns one finite connected admission-to-retirement graph.
Custom Copy execution supplies an ordinary graph with a command-completion result:

```rust
let read = Then::new(PrepareChunk::default(), ReadCopy::default());
let execute = CommandExecution::new(Then::new(read, WriteCopy::default()));
device.hardware_with(execute)
```

A transform composes after ReadCopy inside CommandExecution; chunk-local stages
belong inside that composite. Command-local tails compose after it and preserve
the typed completion result. The constructor does not inspect or rearrange a
caller's Then graph. All branches return to common Retire; callers do not compose
an event publisher or release ownership themselves.

Channel is a persistent hardware object, not a pipeline. It retains the accepted
descriptor ledger, validated traversal and concrete fault checkpoints. Validation,
chunk and payload values move through stage inputs/outputs and local state without
releasing ownership. No Channel stores a Rust Operation, child stage or generic
continuation. No global controller selects child execution phases.

Candidate produces a non-owning task request. FanIn and Arbitrate::snapshots select
a task through DMA's RoundRobin policy. Fetch rechecks the selected head, generation
and eligibility, reads a fresh descriptor, advances consumer and records accepted
ownership and residency. Both fresh and resumed outputs carry the accepted descriptor
with an explicit execution origin. Retained requests use the saved descriptor, never
read the ring again, and resume that same command rather than a successor.
Opcode routing occurs once at this command boundary, directly to the corresponding
execution branch, without an intermediate Copy/Operation family classification.
Copy owns its CheckRanges and installation prefix; resumed Copy skips fresh validation.
The public Copy-validation boundary rejects other opcodes with a retained Routing
error, rather than turning them into a resume bubble. Normal retirement still runs.
The selected execution branch produces a completion or propagates a domain stop
or yielded path, then the common retirement boundary handles ownership.

| Boundary | Transferred data |
| --- | --- |
| Fetch -> command dispatch | Accepted descriptor, fresh/resumed origin and owner identity |
| Copy CheckRanges -> installation | `Flow<CopyProgress>`: validated traversal |
| Installation -> custom Copy execution | `Flow<ChunkInput>`: Copy-only traversal request |
| PrepareChunk -> ReadCopy | `Flow<Chunk>`: spans, contexts and length |
| ReadCopy -> transform/write | `Flow<Payload>`: bounded bytes and their geometry |
| WriteCopy -> CommandExecution | `Flow<Completion>`: successful command completion; no data for an intermediate chunk |
| Execution/custom tail -> FanOut | `Flow<Completion>`: completion, domain stop or yielded path with owner identity |
| FanOut -> Retire | `Vec<(branch_index, Flow<Completion>)>`: completed branch results |
| Retire -> enclosing graph | Unit result after all supplied receipts have been handled |

Values carry private identity metadata and cannot be cloned or forged by an
application. Transform code accesses `Payload::bytes_mut()`; it cannot replace
validated addresses or lengths. `Flow<T>` preserves the identity through data,
bubbles and domain stops. Accepted execution uses an Infallible stage-error port:
domain failures travel as Flow data through Then, so a child error cannot bypass
retirement. Raw driving errors leave the continuation unfinished. Unit output
remains valid in the generic pipeline crate.

Error reporting is a Channel operation, not a separately driven pipeline.
The stage detecting a stopping error calls its generation-guarded owner operation.
Hardware chooses the error code and committed progress; contract constructs the
matching Event code/union representation for both Copy and other commands.
That operation latches stop, writes the owning Event ring and updates IRQ; failed
reporting latches fatal without retry/backpressure. Later stages propagate the
already-reported stop, without fabricating payload or publishing it again.
Success does not touch the Event ring. There is no per-channel Publish stage,
Events FanOut, retained-event merge stage or branch-local reporting tail.

Event visibility does not prove that all pipeline-local work has retired.
Accepted ownership and residency remain until the common Retire boundary consumes
the result, including when software requests reset immediately after the error IRQ.
Terminal success/error releases accepted ownership; a waiting or faulted path
releases residency without discarding its retained command unless lifecycle policy
requires cancellation. Reset completes only after affected resident work has exited.

Then connects the opcode FanOut directly to Retire; there is no Opcodes stage
unwrapping a singleton result. Retire consumes the result collection in order,
retains the first error and still performs necessary cleanup for later receipts.
The branch index is routing metadata, never a channel selector: each Flow carries
its own guarded owner identity. An empty collection completes without effects;
an absent input is not a collection and leaves the stage InProgress.

Execution stages borrow `ExecutionContext`, selecting narrow resources by the
input's Work identity and reset generation. The borrow ends each step. Candidate,
Fetch and Retire use the DMA owner for observation, admission and retirement.
No leaf invokes a previous stage, and context projection performs no lifecycle
cleanup. Custom stages check `Flow::lifecycle(context)` before effects/readiness
and preserve stops while releasing their own local data.

A healthy Copy retains the shared grant across all chunks. CommandExecution
advances its opaque child graph once per step and supplies another chunk only
after the previous child invocation completes. It stays InProgress until the
command ends, parks or stops; it never restarts admission or opcode dispatch for
each chunk. Fill likewise retains its selected execution branch while advancing
its bounded writes. Operand projection happens when that branch accepts input.
False Wait/SEM relinquishes the grant but keeps accepted work, allowing peers
to satisfy the dependency without a PageFault or RESUME obligation.

Payload PageFault parks concrete hardware data in Channel. A read fault retains
the unread Chunk; a write fault retains the already transformed Payload.
ReadCopy and WriteCopy restore their own checkpoints after fresh arbitration,
without refetching, revalidating or replaying successful reads/transforms/writes.
Even immediate RESUME cannot revive the old invocation: it must drain before the
external driver starts the next one. A custom completion tail cannot acknowledge
reset while still holding old-generation data.

Channel source mutations update IRQ causes. Context maintenance observes
acceptance, cursor advancement, suspension and retirement. Fetch discards
unstarted Abort targets within the captured frontier. These are owner operations,
not independently driven lifecycle pipelines.

Reset is an out-of-band hardware request. Idle scopes can clear their storage and
complete immediately. Resident stages observe reset through context in step and
release their own work before completing, independently of data readiness. A
selected scope remains busy until its resident execution has actually exited. Completion
has no minimum latency, but always means that affected old work can no longer act.
Runtime/harness wiring supplies none of this policy. See the
[reset contract](docs/reset.md) for software memory lifetime and the RTL obligation
to drain every affected outstanding transaction.

Fan remains finite N-to-M value flow: child stages own unfinished work, and the fan
owns buffered input/output values and receipts. Policy validates routing before effects;
it never owns device lifetime or fault recovery. Then, FanIn/Arbitrate and
FanOut/Dispatch keep their existing generic behavior. The attempted Producer/Stream
extension is removed: hardware task lifetime belongs to context, not to a
combinator invocation. Repeat and AfterEach remain removed.

Scenarios retain HardwareDriver and use AsyncExecutor::drive. Only the external
driver supplies subsequent finite service inputs and yields after each bounded
interaction. Runtime selects backend and driving policy, never DMA traversal,
arbitration, IRQ or reset rules. Software owns allocation, registration, submission
and repair. Harnesses stop and join hardware when their software program ends.

`StepOutcome` distinguishes only `InProgress` from `Complete(Result<Output, Error>)`.
Stages receive a mutable optional input slot and take it only when accepting work.
Busy stages leave a new offer untouched. There is no public Stage::State or start;
initialization happens inside step and intermediate state stays in the stage.
Completion transfers the result and releases that input's intermediate state.
Only step advances a continuation; no external flush hook exposes child cleanup.
DMA retirement consumes an explicit result after local processing completes;
context projection does not release residency. Software-wait cancellation does not reset hardware.
There are no hardware wait tokens or separate readiness trait. Async driving yields
after every bounded hardware step, including idle polling, so software can access
registers between steps. Software interrupt and doorbell waits remain event-driven.
The retained `Operation` also preserves a completed result across cancellation at
the post-step yield; resuming delivery does not replay the hardware interaction.

`Dma<M, S = ()>` uses `MemoryAccess<S>` with a `DmaAccess` selector containing
channel and context IDs. Plain `Dma::new` keeps unit-sideband memory access.
`Dma::with_config(memory, DmaConfig { channels, capabilities })` and
`with_config_and_sideband(memory, config, sideband)` select immutable atomic widths;
`new` and `with_sideband` default both atomic capabilities off. Discovery is read-only,
and reset preserves these instance capabilities.
Without a context table, `hw-components::windows::ChannelMemoryAccess` resolves the Function
owner. With a table, `ContextMemoryAccess` validates ownership and supplies a
`MemoryIdentity` containing Function and extensible Default/PASID attributes.
The routing adapters' `map_sideband` hook lets a trusted platform retain the
authorized source alongside that identity, including an opaque fault-source tag.
The platform defines and resolves that tag; the MMU only echoes it. PASID/address
alone cannot identify a channel when several channels use the same space. Tag
associations and pending fault handlers must outlive their requests and be retired
before unsafe reuse, as specified in PLAT-PF-02 of
[pageable-copy recovery](docs/page-fault-recovery.md).
Runtime selects the injected identity's backing. DMA remains Function/PASID-unaware.
It neither allocates memory nor chooses
a concrete backing. The scenario harness assembles memory and its allocation service
externally; software allocates its own rings and payload buffers through that service.
`dma/tests/flat_memory.rs` runs the same graph on an independent flat array.

Both MMIO reads and writes are hardware transactions with exclusive device access.
The register owner applies access side effects and refreshes its IRQ state; the runtime
accepts emitted notifications and releases the device lock, preserving the
transaction's success or error. Independently driven hardware steps observe the
resulting state; they do not wait for an MMIO wakeup. The current register set has
no read-clear behavior.

## Shared ring contract

`Ring` is a trait with an associated `Entry` schema. Implementations supply their
base and capacity; default methods compute entry addresses and checked pointer
updates. Entry stride follows the wire size/alignment, not `size_of::<Entry>()`.
`Ring`, `RingPointers` and `RingError` are defined in `pdma-model-contract`.
`RingConfig<Entry>` holds software-selected storage configuration only and lives
in `software::ring` from `pdma-model-software`; host reexports it as `host::RingConfig`.
`RingPointers` is a temporary snapshot acquired by the caller, not cached state.
The trait neither performs I/O nor requires `push`/`pop` methods.

Entry I/O uses the schema-independent `MemoryAccess::read_buffer`/`write_buffer`
defaults on generated raw-byte owners. Ring operations explicitly decode/encode
semantic records, then publish pointers only after successful access and decoding.

| Endpoint | Command ring | Error-event ring |
| --- | --- | --- |
| Host | Write entry, then publish producer through MMIO | Read/decode entry, then publish consumer through MMIO |
| Device | Read/decode entry, then advance its consumer register | Write entry, then advance its producer register |

Device `CommandRing`/`EventRing` implement the trait directly over their own
registers and expose inherent synchronous operations. Host concrete rings delegate
asynchronous publication and consumption to `software::ring::SoftRing`, shared
with firmware.
It borrows register cursors for ordinary DMA and memory cursors for both control
endpoints. Management request IDs and resource policy remain outside SoftRing.
The shared DMA driver retains the bus. Drivers and rings use ordinary Rust ownership;
their state is not bound to the CPU used during construction. Suspended operations
may resume on another CPU or directly through the executor, subject to the supplied
bus/memory capabilities. CPU-specific capabilities and ISR scheduling remain runtime
responsibilities, and CPU ownership tokens stay private to runtime.
Software reads current pointers through that bus; there is no second authoritative
cursor store. `Channel::resume_submission` completes retained publication after
cancellation; it never queues a second command. Control-channel doorbells are
delivered through management ISRs, separate from the DMA vector; see `docs/control-channel.md`.
The [shared software crate](crates/software/README.md) depends on contract, runtime
and schema, with no normal dependency on host, firmware or DMA. An `Entry` parameter does
not make PDMA cursor rules a framework concern. Generic schema/runtime expose no
ring protocol or ring types and have no normal dependency on PDMA crates; generic
transport, CPU and notification mechanics remain in runtime.
The separation follows
[etha_model's register-backed ring operations](https://github.com/shady831213/etha_model/blob/5270f00874a1ad390fbbce6f5395be24a52d0ec9/src/reg_if/ring.rs#L125-L182),
without adopting its raw pointer dereferences or phase-bit pointer encoding.

## Channel behavior

- `Dma::new(memory, channel_count)` fixes the population for that instance;
  `DmaRegisters::new(channel_count)?.resolve()` resolves its generated register bank.
  `DmaDiscovery.channel_num` reports the actual count read-only; neither reset changes it.
  Different counts can coexist in the same program. Host `ChannelConfig { id, rings }` selects a
  channel after reading discovery; setup does not change the device-wide IRQ gate.
  All channels share one notification output, so software checks every channel's
  causes and drains their error events when servicing an interrupt.
- One command ring and one error-event ring, with 32-bit wrapping cursors. Capacities
  are powers of two below 2^31, so slot order remains continuous across counter
  overflow; bases are 8-byte aligned. Geometry freezes on first
  enable, including across disable/re-enable. Software owns allocation and ensures
  rings and payload have appropriate backing and do not alias unintentionally.
- Generated commands contain an ID and a tagged operation payload. Copy has two independent endpoints. Each endpoint is
  either Linear(address,offset) or ScatterGather(list_address,count) with an explicit
  start index and local offset. One command copy_length selects both ranges. Segment records
  contain address/length; ring and list storage uses generated size/alignment metadata,
  while payloads require only byte alignment. See `include/README.md` for the current ABI.
- Hardware validates both selected ranges before payload reads or writes, starting
  directly at each selected SG entry. Unused prefix/suffix entries are not scanned.
  Insufficient selected capacity returns `LengthMismatch`; backing totals need not match.
  List read failures return `ListReadError`; invalid positions or selected-range overflow return
  `InvalidLength`. Software keeps SG descriptors stable across validation and execution;
  hardware retains only the current segment and cursor, not a snapshot of each list.
- Copy iterations use the smaller of the current source remainder, destination remainder,
  command remainder and 256-byte scratch capacity. This scratch size is not a protocol
  limit; there is no additional segment-count/total-length bound. Zero-length segments
  are skipped, zero counts do not read a list, and a zero-length command touches no list or payload.
- Explicitly repairable payload translation misses park the original command,
  both cursors and committed prefix. A failed write retains its successful read
  scratch; resume retries the failed access without replaying committed writes.
  No event, DMA IRQ or firmware event reports this suspension. The platform
  MMU/backend independently reports fault identity/address/access; owner software
  repairs mappings/TLB state, observes 32-bit FAULT_STATUS.faulted and writes
  32-bit RESUME.request=1 through channel work MMIO. Resume clears faulted without
  enabling the channel or clearing errors. Zero and requests while not faulted do
  not prearm a later fault. Same-channel successors wait while peers can run.
  Software serializes fault handling with reset/release/reallocation and retires
  old handlers before reuse; late or duplicate requests are not episode-checked.
  A failed PageFault write commits no bytes
  from that call. See [pageable-copy requirements](docs/page-fault-recovery.md).
- Other payload failures return `ReadError` or `WriteError` and stop further copying. One
  error event reports bytes_completed from successful writes; earlier writes are not rolled
  back. Overlap is neither detected nor rejected. Later reads observe earlier writes
  according to actual chunk order; there is no whole-transfer memmove guarantee.
- Write stores a naturally aligned little-endian 32/64-bit word. Fill repeats the low
  one, four or eight pattern bytes over an arbitrary byte-aligned length, including a
  partial tail. Recoverable target faults retain the accepted command and committed prefix.
- Wait loads the selected word width and evaluates its comparison with acquire semantics
  on success. A false comparison releases the shared grant without setting fault status;
  it retries through arbitration while same-channel successors remain blocked. Reached
  uses signed-width modular subtraction; software maintains distance below half range.
- Atomic performs one width-truncated RMW through the generic memory capability, with no
  software old-value destination. Arithmetic wraps; a CAS mismatch succeeds unchanged.
  Unsupported widths stop before mutation. Semaphore Signal uses release, rejects overflow
  before modification and increments one permit; Wait acquires and consumes only a positive
  count. A bounded CAS mismatch or zero permit count yields for later arbitration.
- Fence writes software-provided completion storage with release scope over
  prior accesses in its channel, including across contexts. It does not drain peers.
  Target PageFault parks for owner resume; permanent write errors stop with WriteError.
  Trap requests notification
  independently of error reporting. Success never creates an Event.
- Timestamp writes a fixed 64-bit external timer sample to an eight-byte-aligned address
  in its selected context. Dma::with_timer connects a read-only platform source; absent
  input reports Unsupported. The managed harness owns a counter advanced per root step,
  not by sampling. Channel/DMA reset does not reset that source. Target faults retain
  the original sample for retry; software uses a later Fence/Trap to observe completion.
  Simulation ticks do not predict physical RTL latency or require timer MMIO.
- Full command ring is a host submission error. Event capacity does not gate command
  admission or successful execution. A stopping error latches stop before event I/O;
  full or invalid event storage is fatal without retry or backpressure.
- Command fetch failure latches CommandRead without consuming the command. Event write
  failure latches EventWrite without advancing its producer; full and invalid geometry
  latch EventFull and EventConfiguration. No successful event write is needed to expose
  fatal status or emit its notification. Reset does not undo earlier payload effects.
- `irq_mask` bit 0 masks event-non-empty notifications and bit 1 masks Trap; one means
  masked. `irq_enable` gates all channel notifications. Error and new fatal notifications
  bypass the routine masks. There is no IOC flag, watermark or success event.
- `irq_cause` is read-only: bit 0 is current event non-empty, bit 1 is sticky Trap,
  bit 2 is sticky error stop, and bit 3 is channel fatal. Writing TRAP_ACK.request=1
  clears only Trap; consuming events never clears error stop. Every unmasked Trap can
  emit even if its sticky cause was already set. Causes remain visible while masked.
- Device-side unmask/enabling never replays historical events. Software must check
  events and errors after enabling before it waits. Software drains through fresh
  producer snapshots until empty, or explicitly schedules further consumption.
  Notification coalescing does not promise one handler call per Trap.
- Hardware emits notifications without knowing software handlers. `take_interrupt`
  hands them to the platform. Runtime retains a pending notification until the existing
  dispatcher claims it; draining, masking or reset cannot retract a delivered event.
  `Model::irq()` reports this platform pending state, not a live device level.
- Channel and device reset use the same request/cleanup/completion handshake: write one
  requests reset, read one means busy, and hardware clears zero after affected accepted
  work retires. Writing zero is ignored; repeated requests coalesce. Busy scopes reject
  configuration writes with `Error::ResetInProgress`, while reads remain available.
  Completion clears channel configuration, pointers, causes and retained work; device
  reset also clears device-wide fatal and controls and preserves `channel_num`.
  It cancels parked work without resume and clears the faulted status.
  The host reset methods consume their driver and poll for zero before returning success;
  software then creates fresh drivers through setup. A cancelled wait or caller timeout
  is not completion: affected memory must stay valid until completion is observed.
  Neither reset clears external memory or rolls back committed writes. Merely disabling
  a healthy channel still suspends and preserves its operation.
  Healthy execution is non-preemptive: disabling the active channel holds the shared unit
  until that channel is re-enabled or reset.
  Resetting one channel neither changes peer registers nor restarts a peer's active command.
  If the reset channel owns the shared engine, retiring its old buffered data and
  unpublished completion can delay peers. Completion guarantees that old work cannot
  make another command, SG-list, payload or event memory access. The model's memory
  calls finish synchronously within a step; it models no outstanding bus transactions
  or timed drain. See [reset semantics and hardware comparison](docs/reset.md).
- `Dma::signal_fatal()` represents a device-wide internal hardware fault input.
  It stops all execution and uses the device control block's independent IRQ enable.
  Only device reset clears it; channel reset does not.
  Ordinary memory-backend diagnostics are not automatically promoted to device fatal.

`ChannelRegisters.recovery.fatal` uses the dedicated `ChannelFatal` register schema. Its
`code` field is `ChannelFatalCode::{None, CommandRead, EventWrite, EventFull, EventConfiguration}`
with wire values 0 through 4. Host `Channel::fatal()` returns this enum and rejects unknown
encodings. Hardware updates the generated field directly; enum values and their
descriptions are also exported to the C header from the same schema metadata.

This is not the predecessor's ABI. DMA channel error-event rings are independent
of the managed PF firmware event transport.
There is no resource credit policy,
old semantic API, custom future, or additional
executor. Historical scenarios are preserved on the backup branch only. The old
C header has been replaced by the current generated contract; see
[`include/README.md`](include/README.md) for generation and consistency checking.

## Validation

```sh
cargo fmt --all -- --check
cargo test --workspace --all-targets
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --workspace --all-features --no-deps --document-private-items
```
