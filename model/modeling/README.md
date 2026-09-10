# Minimal PDMA channel

Run from this directory:

```sh
cargo test -p pdma-model-scenarios --test single_channel -- --nocapture
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

## Read the implementation

| Location | Responsibility |
| --- | --- |
| `crates/contract/src/record.rs` | Generated Segment, Endpoint, Command and Response wire records. |
| `crates/contract/src/registers.rs` | Composed command, response and channel register blocks. |
| `crates/contract/src/ring.rs` | Entry-typed ring geometry and shared pointer rules. |
| `crates/contract/src/bin/pdma-abi-header.rs` | Current C11 header generation through schema exporters. |
| `crates/device/src/channel.rs` | Ring-owned register values, push/pop and channel IRQ state. |
| `crates/device/src/channel/mmio.rs` | Generated block routing and software-write effects. |
| `crates/device/src/stages.rs` | The complete hardware composition. |
| `crates/device/src/execution.rs` | Retained copy progress and chunk payload. |
| `crates/device/src/execution/validate.rs` | Incremental endpoint-total validation. |
| `crates/device/src/execution/cursor.rs` | Shared linear/SG endpoint traversal. |
| `crates/device/src/execution/next.rs` | Select the next chunk across independent boundaries. |
| `crates/device/src/execution/copy.rs` | Composable chunk read/write transforms. |
| `crates/host/src/channel.rs` | Software configuration, submit and response consumption. |
| `crates/host/src/runtime.rs` | PDMA runtime transport: async context access, worker wakeup and IRQ line. |
| `crates/hardware-interface-runtime/src/interrupt/runtime.rs` | CPU-bound software execution, ISR registration and dispatcher lifetime. |
| `crates/scenarios/tests/single_channel/main.rs` | Test suite entry and module declarations. |
| `crates/scenarios/tests/single_channel/cases/` | Linear, scatter/gather and fault cases, with acceptance assertions. |
| `crates/scenarios/tests/single_channel/harness/` | Platform provisioning, fault injection and actor driving/cleanup. |
| `crates/scenarios/tests/single_channel/hardware.rs` | Hardware assembly from the existing device and stage graph. |
| `crates/scenarios/tests/single_channel/software/` | Complete software program, buffer allocation and interrupt handler. |

The device composition is deliberately explicit:

```rust
let chunk = Then::new(Then::new(NextChunk, ReadCopy), WriteCopy);
let execution = Then::new(CheckLengths, Repeat::new(chunk));
AfterEach::new(Then::new(Then::new(Fetch, execution), Publish), UpdateIrq)
```

The outer ports are `() -> Command -> CopyState -> Response -> ()`. The copy body is
`CopyState -> Transfer -> Transfer -> ControlFlow<Response, CopyState>`: Continue feeds
progress into the next iteration, Break finishes the command. Repeat advances one
child step at a time and reuses its hardware; it does not add another executor.
Command execution stages can inspect and transform the owned `Transfer`; `device/tests/composition.rs`
inserts a payload transform between read and write without changing the core or executor.
Command execution
errors travel in the response value; they do not skip publication. Fetch and
publication faults stop the channel. Rings are state in the shared hardware
context, not stages. `Operation` retains accepted work independently of an async
driving future; `StepExecutor` and `AsyncExecutor` drive the same hardware graph.
Use one retained operation/worker per channel. Discarding an accepted operation is
not a modeled hardware cancellation or recovery mechanism.

`Device<M>` depends only on `MemoryAccess`. It neither allocates memory nor chooses
a concrete backing. The scenario harness assembles shared memory and its allocation service
externally; software allocates its own rings and payload buffers through that service.
`device/tests/flat_memory.rs` runs the same graph on an independent flat array.

Both MMIO reads and writes are hardware transactions with exclusive device access.
The register owner applies access side effects and refreshes its IRQ state; the runtime
publishes that level, releases the device lock and notifies the worker to recheck its
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
  backpressure: publication waits with the response retained, without repeating copy.
- Command fetch failure latches fault code 1 without consuming the command. Response
  publication failure latches code 2 without advancing response producer. Payload
  copy may already have completed in the latter case; software must not assume retry
  is safe. No event ring or memory write is needed to expose the fault.
- IRQ level is `irq_enable && (response_pending || fault != 0)`. The existing
  `AfterEach` runs aggregation after each hardware interaction, including faults and
  stalls; MMIO writes update the same equation immediately. Draining responses clears
  their source. Masking clears the line, not the fault. Unmasking a latched source
  reasserts the line. IRQ handlers belong only to host software.
- Fault is read-only and sticky in this first version. Restart/recovery requires a
  fresh channel; there is no ACK/reset/retry command protocol yet. Disabling a healthy
  channel suspends its retained operation without losing committed effects.

`ChannelRegisters.fault` uses the dedicated `ChannelFault` register schema. Its
`code` field is `ChannelFaultCode::{None, CommandRead, ResponseWrite}` with wire
values 0, 1 and 2. Host `Channel::fault()` returns this enum and rejects unknown
encodings. Hardware updates the generated field directly; enum values and their
descriptions are also exported to the C header from the same schema metadata.

This is not the predecessor's ABI. There is no event ring, scheduler, multi-channel
arbitration, resource credit policy, old semantic API, custom future, or additional
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
