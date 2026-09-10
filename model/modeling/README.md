# Minimal PDMA channel

Run from this directory:

```sh
cargo test -p pdma-model-scenarios --test minimal_channel -- --nocapture
```

The system tests run successful copies, including a byte-aligned destination, and a
response-ring write fault. They use
real memory access, the same composed device pipeline, a software CPU, and the
existing interrupt dispatcher. Worker construction/join and assertions belong to
the harness; buffer allocation, payload initialization, software setup, submission, handler registration
and completion waiting run inside one
`InterruptRuntime::run_main` per case, which uses `SoftwareCpu` internally. A channel is transferred once into its persistent
IRQ handler, rather than repeatedly moved through shared optional state.

## Read the implementation

| Location | Responsibility |
| --- | --- |
| `crates/contract/src/record.rs` | Generated command/response wire records. |
| `crates/contract/src/registers.rs` | Composed command, response and channel register blocks. |
| `crates/contract/src/ring.rs` | Entry-typed ring geometry and shared pointer rules. |
| `crates/contract/src/bin/pdma-abi-header.rs` | Current C11 header generation through schema exporters. |
| `crates/device/src/channel.rs` | Ring-owned register values, push/pop and channel IRQ state. |
| `crates/device/src/channel/mmio.rs` | Generated block routing and software-write effects. |
| `crates/device/src/stages.rs` | The complete hardware composition. |
| `crates/device/src/execution.rs` | Read and write transforms with request-owned payload. |
| `crates/host/src/channel.rs` | Software configuration, submit and response consumption. |
| `crates/host/src/runtime.rs` | PDMA runtime transport: async context access, worker wakeup and IRQ line. |
| `crates/hardware-interface-runtime/src/interrupt/runtime.rs` | CPU-bound software execution, ISR registration and dispatcher lifetime. |
| `crates/scenarios/tests/minimal_channel/hardware.rs` | Hardware assembly from the existing device and stage graph. |
| `crates/scenarios/tests/minimal_channel/software.rs` | Complete software program, including buffer allocation and interrupt handler. |
| `crates/scenarios/tests/minimal_channel/main.rs` | Platform provisioning, actor driving and acceptance assertions. |

The device composition is deliberately explicit:

```rust
let execution = Then::new(ReadCopy, WriteCopy);
AfterEach::new(Then::new(Then::new(Fetch, execution), Publish), UpdateIrq)
```

Its ports are `() -> Command -> Transfer -> Response -> ()`. Command execution
stages can inspect and transform the owned `Transfer`; `device/tests/composition.rs`
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

## First-slice behavior

- One command ring and one response ring, with 32-bit wrapping cursors. Capacities
  are powers of two below 2^31, so slot order remains continuous across counter
  overflow; bases are 8-byte aligned. Geometry freezes on first
  enable, including across disable/re-enable. Software owns allocation and ensures
  rings and payload have appropriate backing and do not alias unintentionally.
- Generated little-endian commands contain ID, source, destination and length.
  A copy accepts 0..=256 bytes; zero length touches no payload memory. Larger lengths
  receive `InvalidLength`; inaccessible source/destination receive `ReadFault` or
  `WriteFault`. A failed copy never receives a success response. Reads snapshot the
  payload before writes; no timing or chunked DMA engine is modeled yet.
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
