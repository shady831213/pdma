# Minimal multi-channel PDMA

Next major goal: [firmware-managed DMA](docs/firmware-dma-goal.md), preparing
SR-IOV, PASID and Function reset boundaries while preserving the ordinary
configuration. This is a recorded direction, not implemented functionality.
The first milestone is the [architecture MVP](docs/firmware-dma-mvp.md): an
independent firmware-managed assembly that opens a Host access window and runs
one real copy, while retaining the current direct-DMA scenarios.

Run from this directory:

```sh
cargo test -p pdma-model-scenarios --test single_channel -- --nocapture
cargo test -p pdma-model-scenarios --test multi_channel -- --nocapture
```

The system tests run all four linear/SG endpoint combinations, unequal segment
boundaries, multi-chunk and zero-total transfers, plus response-ring write faults. They use
real memory access, the same composed device pipeline, a software CPU, and the
existing interrupt dispatcher. Worker construction/join belongs to the harness;
case modules own inputs and acceptance assertions. Buffer allocation, payload initialization,
software setup, submission, handler registration
and completion waiting run inside one
`InterruptRuntime::run_main` per case, which uses `SoftwareCpu` internally. A channel is transferred once into its persistent
IRQ handler, rather than repeatedly moved through shared optional state.

Runtime-owned memory installs regions through one atomic `add_regions(&regions)` call.
Overlap and capacity checks, backing allocation and installation run under one exclusive
storage access; an error leaves existing regions and bytes unchanged. There are no public
prepared tokens, transaction drafts or memory revisions. The software allocator advances
its address cursor only after installation succeeds. Hardware still receives only
`MemoryAccess`, without memory ownership or allocation authority.

## Read the implementation

| Location | Responsibility |
| --- | --- |
| `crates/contract/src/record.rs` | Generated Segment, Endpoint, Command and Response wire records. |
| `crates/contract/src/registers.rs` | Composed command, response and channel register blocks. |
| `crates/contract/src/ring.rs` | Entry-typed ring geometry and shared pointer rules. |
| `crates/contract/src/bin/pdma-abi-header.rs` | Current C11 header generation through schema exporters. |
| `crates/dma/src/channel.rs` | Ring-owned register values, push/pop and channel IRQ state. |
| `crates/dma/src/channel/mmio.rs` | Generated block routing and software-write effects. |
| `crates/dma/src/stages.rs` | Hardware stages, single-command pipeline and sustained command loop. |
| `crates/dma/src/fetch.rs` | Per-channel readiness and generation-tagged command acceptance. |
| `crates/hardware-interface-pipeline/src/combinators/` | Typed sequencing, gathering, arbitration and distribution. |
| `crates/dma/src/arbitration.rs` | PDMA round-robin selection policy. |
| `crates/dma/src/routing.rs` | Generation-bound response destination policy. |
| `crates/dma/src/execution.rs` | Retained copy progress and chunk payload. |
| `crates/dma/src/execution/validate.rs` | Incremental endpoint-total validation. |
| `crates/dma/src/execution/cursor.rs` | Shared linear/SG endpoint traversal. |
| `crates/dma/src/execution/next.rs` | Select the next chunk across independent boundaries. |
| `crates/dma/src/execution/copy.rs` | Composable chunk read/write transforms. |
| `crates/host/src/channel.rs` | Software configuration, submit and response consumption. |
| `crates/host/src/runtime.rs` | PDMA transport: async context access, worker wakeup and pending notification delivery. |
| `crates/host/src/channel/control.rs` | CPU-checked software mask, cause and reset access. |
| `crates/dma/src/reset.rs` | Channel-bound task guards around the composed stages. |
| `crates/dma/src/reset/control.rs` | Reset completion after affected accepted work retires. |
| `crates/hardware-interface-runtime/src/interrupt/runtime.rs` | CPU-bound software execution, ISR registration and dispatcher lifetime. |
| `crates/scenarios/tests/single_channel/main.rs` | Test suite entry and module declarations. |
| `crates/scenarios/tests/single_channel/cases/` | Linear, scatter/gather and fault cases, with acceptance assertions. |
| `crates/scenarios/tests/single_channel/harness/` | Platform provisioning, fault injection and actor driving/cleanup. |
| `crates/scenarios/tests/single_channel/hardware.rs` | Hardware assembly from the existing device and stage graph. |
| `crates/scenarios/tests/single_channel/software/` | Complete software program, buffer allocation and interrupt handler. |
| `crates/scenarios/tests/multi_channel/` | Three-channel reset isolation through the same hardware/software/harness separation. |

`device.command_loop()` is the complete hardware assembly entry. Its per-command
composition is deliberately explicit:

```rust
let chunk = Then::new(Then::new(NextChunk, ReadCopy), WriteCopy);
let execution = Then::new(CheckLengths, Repeat::new(chunk));
Then::new(
    Then::new(device.intake(), Resettable::new(AfterEach::new(execution, UpdateIrq))),
    device.responses(),
)
```

Each `Fetch` produces `ChannelTask<Command>`, carrying channel identity and generation.
Its `ReadyStage::ready` checks channel/ring state without consuming a command.
`device.intake()` uses `Arbitrate<Fetch, RoundRobin>`: arbitration specializes the generic
`FanIn` with a singleton plan and identity output, while device owns the selection policy.
`FanIn` itself can gather multiple sources and reduce them into a different output type.
`device.responses()` uses `Dispatch<ResponsePort, RouteResponse>`, the singleton
specialization of `FanOut`. The PDMA `Route` policy supplies only the channel-bound
destination and owned response; successful publication returns unit without an
application reducer. The generic combinators also support application-defined
broadcast and split plans without requiring input cloning. Branches run in plan order;
a pending branch retains its state and completed branches are never replayed.
Both fan combinators use `Then` internally for planning, branch execution and reduction,
sharing the branch and reduction stages. Selection and command acceptance occur in
separate steps. Branch traversal is `Repeat<RunBranch>`, feeding remaining requests and
collected outputs into the next iteration. If a selected channel becomes ineligible before acceptance, arbitration
selects again; work already accepted remains subject to its channel's reset generation.
`Resettable` selects its context and checks the generation before every nested interaction,
retaining `ChannelTask<T>` across output transformations and rejecting a wrong destination.
The shared execution transforms `ChannelTask<Command>` into `ChannelTask<Response>`;
publication consumes that tagged response. The copy body is
`CopyState -> Transfer -> Transfer -> ControlFlow<Response, CopyState>`: Continue feeds
progress into the next iteration, Break finishes the command. Repeat advances one
child step at a time and reuses its hardware; it does not add another executor.
Command execution stages can inspect and transform the owned `Transfer`; `device/tests/composition.rs`
inserts a payload transform between read and write without changing the core or executor.
Command execution
errors travel in the response value; they do not skip publication. Fetch and
publication faults stop the channel. An error response is published before the channel
stops; no subsequent command executes until reset. Rings are state in the shared hardware
context, not stages. `Operation` retains accepted work independently of an async
driving future; `StepExecutor` and `AsyncExecutor` drive the same hardware graph.
`device.command_loop()` builds `CommandLoop = AfterEach<Repeat<RunCommand>, ResetControl>`, reusing one shared
pipeline and one in-flight command for either single-channel or multi-channel devices.
It arbitrates at command boundaries, not between chunks of different channels.
A full idle arbitration round returns Pending; a channel's fault or reset releases
its command, and channels without response space are skipped before acceptance.
`ChannelContext` borrows
only the selected channel and shared memory for each step. A reset write requests cleanup;
hardware keeps the reset register at one until affected accepted work retires, then
clears it to zero. `ResetControl` runs after each command-loop step, including pending
steps. Configuration writes are rejected while the affected scope is resetting.
Generation tags additionally reject retained pre-reset tasks after later reconfiguration;
they do not acknowledge reset. Custom command compositions use `RunCommand<S>`
as their ownership-release boundary and step `ResetControl`. The wrapper accepts any
child stage with the complete command's input/output/error/wait contract and uses the
existing `Operation` to dispose of terminal child state before releasing ownership.
Only termination of the
complete pipeline releases ownership; response publication and `Resettable` binding
do not acknowledge command cleanup.
Discarding an accepted operation does not
model hardware cleanup. See the [reset contract](docs/reset.md) for completion and memory
lifetime requirements.

Build `device.command_loop()` before moving the device into
the runtime transport, and retain that graph across commands. Its lightweight Fetch
ports come from the actual instance population. Custom graphs can pass a subset or
different ordering of Fetch ports to Arbitrate, or supply their own arbitration policy.
Runtime only drives the supplied graph; it never constructs the hardware policy.
Complete scenarios drive one persistent command-loop operation until their software
program finishes, then abort and join the hardware task. Command-local termination
is handled inside the device, not by a scenario-side retry loop. Boundary tests that
need a single command assemble the existing stages in test fixtures.

`Device<M>` depends only on `MemoryAccess`. It neither allocates memory nor chooses
a concrete backing. The scenario harness assembles shared memory and its allocation service
externally; software allocates its own rings and payload buffers through that service.
`device/tests/flat_memory.rs` runs the same graph on an independent flat array.

Both MMIO reads and writes are hardware transactions with exclusive device access.
The register owner applies access side effects and refreshes its IRQ state; the runtime
accepts emitted notifications, releases the device lock and notifies the worker to recheck its
conditions, preserving the transaction's success or error. Ordinary reads may therefore
cause an extra recheck. The current register set has no read-clear behavior.

## Shared ring contract

`Ring` is a trait with an associated `Entry` schema. Implementations supply their
base and capacity; default methods compute entry addresses and checked pointer
updates. Entry stride follows the wire size/alignment, not `size_of::<Entry>()`.
Host's `RingConfig<Entry>` holds software-selected storage configuration only;
contract does not define or store software configuration.
`RingPointers` is a temporary snapshot acquired by the caller, not cached state.
The trait neither performs I/O nor requires `push`/`pop` methods.

Entry I/O uses the schema-independent `MemoryAccess::read_typed`/`write_typed`
defaults on generated raw-byte owners. Ring operations explicitly decode/encode
semantic records, then publish pointers only after successful access and decoding.

| Endpoint | Command ring | Response ring |
| --- | --- | --- |
| Host | Write entry, then publish producer through MMIO | Read/decode entry, then publish consumer through MMIO |
| Device | Read/decode entry, then advance its consumer register | Write entry, then advance its producer register |

Device `CommandRing`/`ResponseRing` implement the trait directly over their own
registers and expose inherent synchronous operations. Host concrete rings implement
the same trait and expose inherent asynchronous operations; `Channel` delegates to
them. One private host register port retains the bus and delegates per-poll CPU
validation to the runtime's `CpuOwner::scope`.
Software reads current pointers through that bus; there is no second authoritative
cursor store. The separation follows
[etha_model's register-backed ring operations](https://github.com/shady831213/etha_model/blob/5270f00874a1ad390fbbce6f5395be24a52d0ec9/src/reg_if/ring.rs#L125-L182),
without adopting its raw pointer dereferences or phase-bit pointer encoding.

## Channel behavior

- `Device::new(memory, channel_count)` fixes the population for that instance;
  `DeviceRegisters::resolve(channel_count)` composes its generated register blocks.
  `DeviceControl.channel_num` reports the actual count read-only; neither reset changes it.
  Different counts can coexist in the same program. Host `ChannelConfig { id, rings }` selects a
  channel after reading discovery; setup does not change the device-wide IRQ gate.
  All channels share one notification output, so software checks every channel's
  causes and drains their responses when servicing an interrupt.
- One command ring and one response ring, with 32-bit wrapping cursors. Capacities
  are powers of two below 2^31, so slot order remains continuous across counter
  overflow; bases are 8-byte aligned. Geometry freezes on first
  enable, including across disable/re-enable. Software owns allocation and ensures
  rings and payload have appropriate backing and do not alias unintentionally.
- Generated commands contain an ID and two independent endpoints. Each endpoint is
  either Linear(address,length) or ScatterGather(list_address,count). Segment records
  contain address/length; ring and list storage uses generated size/alignment metadata,
  while payloads require only byte alignment. See `include/README.md` for the current ABI.
- Hardware first traverses both descriptions and sums lengths with checked arithmetic.
  Mismatched totals return `LengthMismatch` before payload reads or writes. List read
  failures return `ListReadFault`; length/list-address arithmetic overflow returns
  `InvalidLength`. Software keeps SG descriptors stable across validation and execution;
  hardware retains only the current segment and cursor, not a snapshot of each list.
- Copy iterations use the smaller of the current source remainder, destination remainder,
  command remainder and 256-byte scratch capacity. This scratch size is not a protocol
  limit; there is no additional segment-count/total-length bound. Zero-length segments
  are skipped, zero counts do not read a list, and a zero-total command touches no payload.
- Payload failures return `ReadFault` or `WriteFault` and stop further copying. One
  response reports bytes_completed from successful writes; earlier writes are not rolled
  back. Overlap is neither detected nor rejected. Later reads observe earlier writes
  according to actual chunk order; there is no whole-transfer memmove guarantee.
- Full command ring is a host submission error. Full response ring is hardware
  backpressure: arbitration leaves that channel's commands queued and serves eligible peers.
  With one in-flight command, a free response slot cannot be taken by another command
  before publication. The composable Publish stage still retains a response if driven
  independently against a full ring.
- Command fetch failure latches fault code 1 without consuming the command. Response
  publication failure latches code 2 without advancing response producer. Payload
  copy may already have completed in the latter case; software must not assume retry
  is safe. No event ring or memory write is needed to expose the fault.
- Successful publication notifies on response empty-to-non-empty transitions and,
  independently, when the command's IOC flag is set. `irq_mask` bit 0 masks the
  first source and bit 1 masks IOC; one means masked. Both masks reset to zero.
  `irq_enable` gates all channel notifications. There is no watermark. Software can
  use IOC-only batches, but must leave enough response space to reach the IOC entry.
- `irq_cause` is read-only: bit 0 is current response non-empty, bit 1 means an
  unconsumed IOC response exists, bit 2 is sticky response-error stop, bit 3 is
  channel fatal. IOC clears when consumer passes the latest IOC response, even if
  ordinary responses remain. Causes are visible regardless of notification masks.
  There is no W1C ACK or exposed IOC task pointer. Error responses and new channel
  fatal events bypass the two routine masks, including when response is already non-empty.
- Device-side unmask/enabling never replays historical events. Software must check
  responses and errors after enabling before it waits. Software drains through fresh
  producer snapshots until empty, or explicitly schedules further consumption.
  Notification coalescing does not promise one handler call per IOC.
- Hardware emits notifications without knowing software handlers. `take_interrupt`
  hands them to the platform. Runtime retains a pending notification until the existing
  dispatcher claims it; draining, masking or reset cannot retract a delivered event.
  `Model::irq()` reports this platform pending state, not a live device level.
- Channel and device reset use the same request/cleanup/completion handshake: write one
  requests reset, read one means busy, and hardware clears zero after affected accepted
  work retires. Writing zero is ignored; repeated requests coalesce. Busy scopes reject
  configuration writes with `Error::ResetInProgress`, while reads remain available.
  Completion clears channel configuration, pointers, causes and retained work; device
  reset also clears device-wide fault and controls and preserves `channel_num`.
  The host reset methods consume their driver and poll for zero before returning success;
  software then creates fresh drivers through setup. A cancelled wait or caller timeout
  is not completion: affected memory must stay valid until completion is observed.
  Neither reset clears external memory or rolls back committed writes. Merely disabling
  a healthy channel still suspends and preserves its operation.
  Execution is non-preemptive: disabling the active channel holds the shared unit
  until that channel is re-enabled or reset.
  Resetting one channel neither changes peer registers nor restarts a peer's active command.
  If the reset channel owns the shared engine, retiring its old buffered data and
  unpublished response can delay peers. Completion guarantees that old work cannot
  make another command, SG-list, payload or response memory access. The model's memory
  calls finish synchronously within a step; it models no outstanding bus transactions
  or timed drain. See [reset semantics and hardware comparison](docs/reset.md).
- `Device::signal_fatal()` represents a device-wide internal hardware fault input.
  It stops all execution and uses the device control block's independent IRQ enable.
  Only device reset clears it; channel reset does not. The minimal model has no
  autonomous shared-engine fault source, and memory-backend diagnostics are not
  automatically promoted to device-wide fatal.

`ChannelRegisters.fault` uses the dedicated `ChannelFault` register schema. Its
`code` field is `ChannelFaultCode::{None, CommandRead, ResponseWrite}` with wire
values 0, 1 and 2. Host `Channel::fault()` returns this enum and rejects unknown
encodings. Hardware updates the generated field directly; enum values and their
descriptions are also exported to the C header from the same schema metadata.

This is not the predecessor's ABI. There is no event ring, resource credit policy,
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
