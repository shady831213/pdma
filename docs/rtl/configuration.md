# DMA elaboration configuration

`DmaCopy` connects the register frontend, `DmaControl`, `DmaCommandPath`, clock crossing and memory endpoint. Within the command path, each channel owns descriptor reservations and the shared engine selects a ready head.
An alternative engine implements the same channel-resource and memory protocols;
it does not have to reproduce the register bridge, notifications or CDC queues.
Channel registers remain owned by the channel register modules. Engine-private
accepted commands and retained payloads are not shadow copies of those registers.

## Engine integration contract

`DmaChannels` is a typed wiring interface, not another register bank. The control
plane supplies current ring/configuration values, one-cycle software actions,
event capacity/geometry and the global stop/reset signals. The engine supplies
channel status, command/event pointer advances, reset completion and its retained
internal-fatal state. Its channel count must match the control plane and interface.

An alternative engine preserves these obligations:

- `command_advance` accepts a descriptor; it does not report command completion.
  `event_advance` publishes a fully written error record. Invalid/full event
  storage follows the error contract, rather than silently stalling admission.
- Reset first stops new work for its scope. `clear` acknowledges that the owner
  has discharged every accepted transaction and retained responsibility for that
  channel. The control plane collects those acknowledgements for global reset;
  it does not inspect private engine state or manufacture a drain completion.
- `CopyMemoryPort` accepts a complete bounded request on `valid && ready` and
  returns one terminal receipt on `returned && accepted`. Identity survives CDC
  and external transaction completion. Acceptance creates the drain obligation,
  even when the AXI address has not yet left the queue.
- Slot zero belongs to descriptor reads. The engine uses slots 1..SLOTS for retained Copy rows and SLOTS+1 for Fence/event writes. At its boundary the engine receives one head and a valid indication per channel, plus a per-channel fetch-outstanding indication for reset. A terminal receipt retains its original tag.

Integration selects one implementation statically in the top-level composition.
It instantiates the shared channel interface and memory interface with the same
Veryl type arguments at each endpoint. No runtime engine selector, extra FIFO or
compatibility wrapper is required. Replacing an engine still requires fresh
functional and implementation qualification; the interface alone does not prove
its lifecycle behavior or timing.

## Replacing a bus endpoint

The configuration port terminates at `common_cell::AxiLiteRegisterFrontend`.
A replacement register target (for example, an APB slave) drives the same
`RegisterPort` in the register clock domain. It must hold an offered operation
through backpressure, return one host response per accepted operation, and
preserve byte lanes and access status. The existing mailbox and register owners
then perform the operation and its side effects exactly once.

The memory clock crossing carries DMA requests and receipts. In the bus domain
`CopyMemoryEndpoint` packs the channel and context selector into address
attributes and classifies response USER. The DMA-local read and write endpoints
own AXI handshakes, beat assembly, directional credits and receipt merging.
A replacement in the bus clock domain services the descriptor address port and the execution `CopyMemoryPort`, and merges their terminal receipts. It must retain the caller's tag and return the terminal outcome
even when the channel enters reset after admission. The clock crossing retains custody in independent descriptor and execution request FIFOs and one return FIFO.

## Inputs and dependencies

`rtl/profiles/default.json` defines the development integration profile. An override
JSON supplies any subset of these independent inputs. None is software writable.

| Input | Meaning | Default |
| --- | --- | --- |
| `channels` | Physical channels, 1..8 | 2 |
| `prefetch_depth` | Descriptor reservations per channel, including queued and partially returned descriptors | 2 |
| `register_bits` | AXI4-Lite data width, 32 or 64 | 32 |
| `memory_axi_version` | 4 selects the AXI4 port; 5 also exposes the six-bit AWATOP output, tied low for ordinary transfers | 5 |
| `memory_bits` | Ordinary AXI data width, 32/64/128/256/512/1024 | 64 |
| `burst_beats` | Maximum beats in one retained memory request, 1..256 | 8 |
| `slots` | Burst rows reserved for each channel | 8 |
| `read_outstanding` | AXI read tracking capacity | 8 |
| `write_outstanding` | AXI write tracking capacity | 8 |
| `request_depth` | Core-to-bus async FIFO depth | 8 |
| `receipt_depth` | Bus-to-core async FIFO depth | 8 |

Async FIFO depths are powers of two and at least four, matching the pinned
`common_cell` constraint contract. Slots and directional
tracking capacities need not be powers of two. The maximum retained request must
hold at least eight bytes, including on a 32-bit memory port. Burst splitting
still enforces the AXI 4KiB boundary and actual transfer alignment.

The profile resolver derives bus byte counts, retained-burst bytes, RAM width,
RAM row count and slot-tag width. RTL derives pointer/counter widths and uses an
explicit wrap for non-binary slot counts. The retained RAM is one
`channels * slots` by `memory_bits * burst_beats` 1R1W resource; the per-channel
partition does not imply one physical memory or engine per channel. Slot zero
identifies fetch traffic; slot tags cover `0..slots+1`. AXI ID width is derived as `ceil(log2(2*channels))`.

These capacities are independent because they cover different lifetimes:
retained rows survive faults; AXI credits cover accepted external transactions;
CDC queues cover transfers crossing clock domains. Making one larger does not
automatically require the others to match. Smaller capacities may impose real
backpressure. Legality does not promise a throughput or physical timing result;
those require matched configuration-specific qualification.

Capacity choices also have different physical costs. The retained Copy RAM grows
as `channels * slots * memory_bits * burst_beats`, alongside per-row metadata and
selection logic. Request/receipt CDC storage grows with its own depth and retained
burst width. Write tracking retains queued payloads as well as addresses and tags;
read tracking retains metadata, with separate partial bursts for interleaving IDs. Increasing
read credits therefore does not have the same storage cost as increasing write
credits. The control-budget synthesis model excludes only the inventoried Copy
RAM; FIFO storage and endpoint assembly remain synthesized logic.

These knobs can hide platform latency by allowing more work in flight, but cannot
create another bus port or engine. The current shared request FIFO still has
head-of-line blocking, and the AXI endpoint preserves same-ID response order and accepts cross-ID interleaving/reordering. Capacity changes need workload-specific throughput and timing
checks; larger values are not unconditionally better.

Register addresses remain 32 bits, memory addresses 64 bits, and selectors 32
bits. The external address sideband retains the contract's selector plus channel
identity. Those protocol widths are not free capacity settings. Descriptor and
register layout, error/fault behavior and software reset obligations are unchanged.
SG and the other command families remain outside this RTL slice.

## One resolved profile

```sh
bash rtl/flow/snapshot.sh "$PDMA_RUN_ROOT/snapshot" profile.json
bash verification/dma/flow/compile.sh "$PDMA_RUN_ROOT/snapshot" "$PDMA_RUN_ROOT/build"
bash impl/dma/flow/genus.sh "$PDMA_RUN_ROOT/snapshot" "$PDMA_RUN_ROOT/synthesis"
```

The DMA wrapper creates a native integration project that references the real
RTL sources and existing lockfile. Its `DmaConfigured` module selects native
Veryl type arguments and contains only one `DmaCopy` instance. Veryl still owns
source discovery, dependency resolution and emitted file ordering. No emitted RTL
is rewritten, and the design sources are not copied into an alternate tree.

The immutable snapshot retains the resolved configuration alongside its RTL.
The VIP obtains interface names from the compiler's actual output, data widths
and channel counts from this configuration, and independent expected payloads
from its scenario objects. Checkers size their identity tracking from the same
slot count. Implementation derives ports, RAM geometry and FIFO constraints from
the snapshot rather than a second editable set of dimensions. A later conflicting
channel override is rejected. Each different integration profile requires its
own snapshot and qualification results.

## Qualification status

Functional checks cover every count in 1..8, the default 64-bit memory profile,
a 64-bit register/128-bit memory profile with three channels and three slots, and
a 32-bit memory profile with one channel, one slot and three-beat bursts. The
ten configurations passed 408 positive scenario runs and rejected eight checker
mutations per configuration on the November 19 testbench. The default
2/8-channel throughput comparison retains the
baseline's zero payload bubbles and measured command latency. Fresh source-bound
CDC/RDC gates pass at both counts with zero unresolved findings. See the
[verification record](../../verification/dma/results.md) for exact coverage.

At checkpoint `4de9cb7f`, the default two- and eight-channel profiles and the
narrow one-channel profile met synthesized setup. Both default profiles passed
the restored-database audits. The three-channel profile with 128-bit memory
missed core setup by 43.2ps. At `9500c28c`, all four profiles met setup
and passed the restored-database audits.
Additional focused functional regressions exercise 256-, 512- and 1024-bit
memory ports and a 256-beat burst configuration.
The [verification record](../../verification/dma/results.md) lists the tested
combinations. These four profiles have no synthesis qualification. The
capacitance-only RAM boundary still excludes physical SRAM area, power and
intrinsic timing. New profile legality is not physical signoff.
