# Native block tests

Place Veryl native tests here, grouped by the hardware block they exercise.
Keep stimulus, clock/reset control, independent expectations and assertions visible
in each test. Shared support must not become a second hardware executor.
`registers/` covers access, storage and the CSR bridge; `execution/` covers command
ownership and the burst window; `memory/` covers endpoint transfers and streaming.

The DMA register and layout tests cover:

| Test | Observable behavior |
| --- | --- |
| `DmaLayoutTest` | Schema offsets and parameterized admin/channel page boundary |
| `RegisterAccessTest` | 32/64-bit lane mapping, full/half configuration access, invalid strobes, permissions and reserved bits |
| `RegisterCdcTest` | Independent clock advancement, startup and request/response backpressure without duplicate transfer |
| `RingRegistersTest` | Software/hardware cursor ownership, publication bounds, configuration freeze and cleanup |
| `DmaRegisterBankTest` | Three-channel isolation, live reset status, request versus cleanup completion, Trap set versus W1C priority |
| `DmaRegisterInterfaceTest` | Complete 32- and 64-bit AXI→CDC→owner paths, nonzero aperture base, read snapshot retention and exactly-once Resume under response backpressure |

The SG tests drive the accepted descriptor and memory receipt interfaces directly.
`ScatterGatherCopyTest` checks all four endpoint combinations with different
source/destination boundaries, the committed prefix on a late write error, and
retirement of a zero-length SG Copy without a metadata access. `SegmentResetTest`
holds a metadata read outstanding across reset, returns a failed receipt and then
executes a new Trap on the same channel.

The minimal execution slice adds `CopyEngineTest`, `CopyErrorTest` and
`CopyFaultTest`, plus `FenceTest`, `FenceOrderTest` and `FenceMemoryTest`.
`FenceTest` exercises scalar Write, Fence and Timestamp against the same memory
port, including a zero timer sample held across a PageFault. `FillExecutionTest`
checks little-endian patterns across page and burst boundaries, malformed entries
and the zero-length and overflowing cases. `TrapExecutionTest` holds the preceding Fence response while a Trap is ready,
checks the execution pulse on acceptance, and rejects a Trap with nonzero reserved bytes.
`MemoryStreamTest` checks continuous writes with delayed B responses;
`CopyWindowTest` checks multi-burst fault/error/reset lifetime;
`CopyWindowReuseTest` checks receipt-driven slot reuse; `CopyWindowForwardTest`
checks direct read-to-write handoff and its reset debt. Descriptor boundary,
channel discovery, peer progress and independent reset fixtures retain their
separate checks.
Their scope is recorded in the
[slice description](../../../docs/rtl/minimal-copy.md#verification-and-limits).

The configurable-resource tests exercise directional credits, retained rows and Fence ordering.

| Test | Observable behavior |
| --- | --- |
| `FetchWindowTest` | Withhold R until two complete descriptors have issued; verify capacity, ordered error delivery and reset |
| `IndependentFetchTest` | Hold A’s fetch returns while B completes a Fence and reset A between its terminal responses |
| `FetchArbitrationTest` | Execution wins a free AR; a stalled descriptor AR retains priority and permits an independent write |
| `ReceiptPriorityTest` | Simultaneous R/B after an uncontended read; the next RLAST transfers back-to-back and the deferred B receives the next grant after backpressure |
| `MemoryCapacityTest` | Independent one-read/three-write credits, held terminal responses and ordered tag/credit return |
| `WindowSingleTest`, `WindowThreeTest` | One and three retained rows, wrap/reuse beyond one window and terminal drain |
| `NarrowFenceOrderTest` | 32-bit memory, three retained rows, bounded descriptor fetch and two-beat Fence64 after Copy |

Standalone AXI4-Lite pairing is checked in the shared register cell. DMA-local `tests/memory` exercises wide byte packing, mixed errors, unowned replies, interleaved R beats and reordered B responses.

Run both native CC and Cranelift backends with `VERYL_DUT_REUSE=0`, as shown in
[the RTL README](../../README.md). These fixtures drive lifecycle and IRQ status
inputs explicitly. The Copy tests instead connect the real execution/reset owner
and memory endpoint. Neither set proves the complete DMA or all outstanding-window
configurations. Clock scheduling tests do not simulate
metastability or substitute for CDC/RDC analysis and physical timing checks.
