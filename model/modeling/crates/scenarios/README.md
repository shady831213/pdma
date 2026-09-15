# Current PDMA scenarios

This crate assembles the current contract, device, host and reusable building blocks
into system acceptance tests. It contains no hardware implementation or driver API.
The predecessor's scenarios remain on `backup/pdma-before-rebuild-20260909` only.

Run from `model/modeling`:

```sh
cargo test -p pdma-model-scenarios --test single_channel -- --nocapture
cargo test -p pdma-model-scenarios --test multi_channel -- --nocapture
```

`single_channel` constructs a device with one channel, with a command
ring, a response ring and IRQ delivery. Its cases cover linear/SG copies and failures
on that same channel:

- `copy_completes_through_host_irq_delivery`: copied bytes, a Success response,
  one IRQ delivery and no remaining platform notification after its claim.
- `copy_accepts_a_byte_aligned_destination`: a nine-byte payload is copied into
  a destination that is not eight-byte aligned, with normal response and IRQ delivery.
- `response_write_fault_reaches_host_without_a_response_entry`: the copy completes,
  response publication fails, and the host observes `ChannelFaultCode::ResponseWrite`
  through IRQ delivery without needing a response entry.
- `software_setup_failure_stops_interrupt_runtime`: an unmapped command buffer
  fails submission before ISR registration, and the runtime returns without hanging.
- `software_allocation_failure_stops_interrupt_runtime`: software receives an exhausted
  allocator and returns its allocation error without installing memory or delivering IRQs.
- `all_endpoint_combinations_copy_across_independent_segment_boundaries`: linear/linear,
  linear/SG, SG/linear and SG/SG transfers preserve the byte stream, including source
  segments `[3, 0, 5]` and destination segments `[2, 6]`.
- `scatter_gather_copy_exceeds_the_internal_chunk_size`: a 769-byte command crosses
  independent segment and internal chunk boundaries, including zero-length segments.
- `zero_total_endpoints_complete_without_payload_mappings`: all nine pairs of linear,
  empty-list and all-zero-list endpoints complete with zero bytes and normal IRQ
  delivery, despite zero-length allocations having no payload backing.

Every successful case verifies `bytes_completed`, the actual destination backing
bytes, one IRQ delivery and consumption of the platform notification.

`recovery/` adds IOC-only batched copies and error-response recovery in the same
runtime. Its software owns allocations, handler registration, response draining and
reset/reconfiguration; its hardware module assembles/drives the pipeline and joins
worker teardown. The batch selects IOC on its last command; the recovery case masks
both routine notification sources and proves error notification still arrives.

## Firmware-managed architecture MVP

`tests/firmware_managed` contains cases and harness support only. It uses the
production `managed-runtime` assembly, `hw-components` access window and `firmware`
program rather than implementing them in the tests. One Host program probes denied
access before boot, waits for firmware readiness, allocates its own buffers and
performs a real IRQ-driven copy. A second case ends with an early Host error and
checks that no system actor retains memory ownership. The initial startup signal
only controls test scheduling; firmware alone opens the hardware window.

```sh
cargo test -p pdma-model-scenarios --test firmware_managed -- --nocapture
```

## Test structure

```text
tests/single_channel/
├── main.rs
├── cases/
│   ├── mod.rs
│   ├── linear.rs
│   ├── scatter_gather.rs
│   └── faults.rs
├── harness/
│   ├── mod.rs
│   └── fault.rs
├── hardware.rs
└── software/
    ├── mod.rs
    └── buffers.rs
```

Paths below are relative to `tests/single_channel/`.

| File | Responsibility | Boundary |
| --- | --- | --- |
| `main.rs` | Declare suite modules. | Contains no setup or test logic. |
| `cases/` | Select inputs and assert observable outcomes. | Calls the harness; contains no platform or actor assembly. |
| `hardware.rs` | Assemble `Device` and its existing pipeline. | Receives a memory capability; knows no software CPU, driver or handler. |
| `software/mod.rs` | Allocate rings and payload buffers, initialize data, configure queues, submit, register the ISR and await its result. | Receives allocator, bus/memory access and ISR registration; has no device core, executor or dispatcher access. |
| `software/buffers.rs` | Build either endpoint from its requested buffer layout. | Software allocates and initializes byte-aligned payload spans and aligned, schema-encoded SG lists. |
| `harness/mod.rs` | Select backing and allocation service, drive hardware, run software and collect results. | Owns actor cleanup; does not allocate application buffers or construct a handler registry. |
| `harness/fault.rs` | Reject a ring write through a test-only memory view. | Observes the actual software-programmed base through a forwarding bus; no fixed buffer addresses or allocation-order assumptions. |
| `hardware-interface-runtime::InterruptRuntime` | Run the program and dispatch interrupts on its CPU. | Owns the handler table, CPU context and dispatch lifetime. |

The harness makes the execution boundary explicit:

```rust
let (device, stage) = hardware::assemble(memory.clone())?;
let model = Model::new(device);
let runtime = InterruptRuntime::new(cpu, model.clone());
let (completed, summary) = runtime.run_main(async |interrupts| {
    let ports = software::Ports { bus: model.clone(), memory: memory.clone(), allocator, interrupts };
    software::main(ports, software::Input::linear(b"dma-copy")).await
}).await?;
```

Each case has one complete program inside one `run_main`. Software installs its
persistent ISR with `ports.interrupts.register((), handler)?`; the runtime enforces
CPU ownership. Software has no CPU token, registry builder, freeze step or registry
handoff channel. The remaining completion oneshot is ordinary software communication
from the ISR to the waiting program, not runtime startup wiring.

The current program transfers its channel into the ISR immediately after submission,
in the same CPU poll as this transport's producer write. This is not a general rule
for drivers whose bus can suspend after committing a write: those drivers need to
install their ISR before enabling/submitting work. The runtime dispatches live IRQs
and reports unregistered vectors rather than silently buffering them.

On program return the runtime stops dispatch and waits for an accepted ISR to finish.
An ISR/source failure ends the waiting program with the concrete dispatch error.
Dropping the whole scoped runtime cancels its software futures without leaving tasks.
CPU rejection, duplicate registration and lifecycle tests belong to the runtime crate.

Before the software starts, backing contains no allocated regions. The software calls
the existing `SharedMemoryAllocator` for both rings and the source/destination buffers,
then configures and uses them. `software::Input` supplies the payload bytes and
independent `BufferLayout` values for source and destination: `Linear` or
`ScatterGather(&[segment_lengths])`. The shared software builder allocates each
payload span, initializes it, and encodes the SG list using `Segment::to_bytes()`.
The software returns destination spans alongside its ISR result; the harness returns
these results and backing to the case. Shared assertions in `cases/mod.rs` read each
span and compare their concatenation with the input stream.
Ring allocation uses each record's generated `RECORD_SIZE` (including tail padding)
times capacity and its `RECORD_ALIGNMENT`. SG list allocation uses the generated
`Segment` record geometry. Payload spans use `MemoryAlignment::ONE`; endpoint lengths
come from software's allocations. Hardware's internal chunk size is not a protocol limit.

Fault cases select a generated ring-base register in the harness. The forwarding bus
records the address software writes there, and the injected memory view rejects writes
at that entry address with `MemoryAccessError::Unmapped`. Both software and hardware use
that view over the allocator's real backing. Software runs the same allocation path in
all cases and has no fault-mode branches. Hardware behavior and driving still come from
the existing device and pipeline crates.
These tests run in ordinary `cargo test --workspace`; crate-local boundary tests
stay in contract, device, host and the building-block crates.

## Multi-channel reset isolation

`tests/multi_channel/` constructs a three-channel device and drives its shared DMA execution stage through the
existing async executor. Each case runs one complete software program inside one
`InterruptRuntime::run_main`. Software discovers the channel count, allocates all
three channels' independent rings and payloads, registers the shared IRQ handler,
submits work, resets hardware through its selected host driver, and drains every
active channel after a shared notification.

The cases cover:

- A reset after A's first committed payload write while B/C remain queued. A's old
  destination stays identical to its pre-reset snapshot while B/C and fresh A complete.
- A queued B reset while A is copying. A and C complete exactly, B's original
  destination stays untouched, and fresh B work succeeds.
- A full one-entry response ring while B/C finish. A's second command remains
  unconsumed and its destination untouched; resetting A discards the old response
  and queued command before fresh A work completes.
- A device reset while A is copying and B/C remain queued. A's committed prefix
  stays unchanged and B/C remain untouched; only fresh A work completes.

Every case checks the exact completion IDs and byte counts. Software also verifies
that channel-count discovery rejects writes and preserves its instance value across reset.

The software program receives memory-write notifications from a test-only forwarding
memory capability. It waits for observable payload changes without inspecting device
internals or using elapsed-time delays. `hardware.rs` assembles and drives hardware;
`harness.rs` selects the backing and owns joined worker teardown; `software/` owns the
complete program and application buffers; `cases.rs` owns acceptance assertions.
