# Common hardware cells

This native Veryl library has no PDMA/PCIe protocol dependency. It is a local
snapshot pending extraction of the two projects' shared cells into a separate
repository. The project name and public interfaces are retained so extraction
can change dependency location without changing hardware consumers.

## Source and changes

Imported sources and tests come from
[pcie_modeling at 67989b3e](https://github.com/shady831213/pcie_modeling/tree/67989b3ebbdc8466e79ded2c65ad64c0da5585b0/rtl/common_cell).
The matching [generic RAM providers](https://github.com/shady831213/pcie_modeling/tree/67989b3ebbdc8466e79ded2c65ad64c0da5585b0/rtl/tech_cell/generic)
live separately under `../tech_cell/generic`. `upstream.sha256` records the imported
files relative to `rtl`. The original cells and tests are unchanged. The README
is PDMA-specific; `DuplexMailbox` and its test are additions extracted from PDMA.
Updates must be deliberate, with provenance and regression results; there is no
automatic synchronization or dependency on an external checkout.
The instance-scoped `constraints/duplex_mailbox.tcl` is also a PDMA addition;
it owns no platform clocks, global clock groups, external IO budgets or reset policy.

## Boundaries and use

| Cells | Contract | Current use |
| --- | --- | --- |
| `ReadyValidSlice` | Typed single-clock Bypass/Forward/Reverse/Full transport | RegisterFrontend AW/W/AR holding registers use Forward |
| `BoundedIndex`, `RoundRobin` | Pure bounded selection; ownership/cursor remain with caller | Available for later multi-channel composition |
| `Ram1RW`, `Ram1R1W`, `Ram1R1WAsync` | Typed RAM ports and provider contracts; optional same-clock forwarding | Provider-independent storage building blocks; not forced into current scratch |
| `DuplexMailbox` | Two typed acknowledged CDC mailboxes with independent release and bounded custody | Register and memory request/receipt crossings |

Slice modes have capacity/empty latency of 0/0 (Bypass), 1/1 (Forward), 1/0
(Reverse), and 2/1 (Full), with unstalled rate one beat per clock. Reset clears
custody, not a protocol obligation; invalid payload values are unspecified. The
consumer owns credit until its actual protocol completion, so the register
frontend still prevents replacement before the matching AXI response is consumed.

RAM providers have synchronous reads, no content reset and bit masks. Dimensions
and addresses must be valid. Same-address read/write collisions are excluded
unless the same-clock wrapper explicitly enables full-write forwarding; partial
mask collisions are not enabled by that option. The asynchronous RAM wrapper is
dual-clock storage, not an asynchronous FIFO or a clock-domain ownership protocol.

`DuplexMailbox #(REQUEST_TYPE, RESPONSE_TYPE)` exposes source/destination clock,
reset, valid/ready and typed request/response ports. Each direction stores one
message using the standard-library acknowledged handshake. The directions are
independent: the cell does not associate responses with requests, count active
transactions or implement drain/cancellation. Both resets must assert together
and release synchronously to their clocks. A released sender may buffer one
message while its peer is still reset; delivery waits for that peer to run, and
the sender retains the message until acknowledgement. Independent warm reset is
not supported. Do not drive these reset ports with a software channel reset. Owners close admission
and drain obligations before clearing their own state.

Use the [reusable timing API](../../impl/pdma/constraints/README.md) for synthesis
and integration. It accepts the mailbox instance, actual clock objects and an
explicit maximum-delay budget. CDC structural review does not replace applying
and checking those physical constraints.

PDMA wire decoding, ring geometry, AXI arbitration/credit, fault handling and reset
policy stay in `src/dma`. A helper belongs here only when its interface and
semantics are independent of those policies; repeated syntax alone is insufficient.

## Verification

From `rtl/common_cell/tests`:

```sh
VERYL_DUT_REUSE=0 bash ../../../tools/rtl-flow/veryl.sh test
VERYL_DUT_REUSE=0 bash ../../../tools/rtl-flow/veryl.sh test --backend cranelift
```

The imported tests cover slice modes, custody/pressure, selection, RAM masks,
depths, forwarding and clock ownership. The mailbox test checks independent wide
payload types, retained snapshots, backpressure and no duplicate delivery.
The startup test covers both release orders and early custody with a reset peer.
DMA integration tests remain in `rtl/src/tests`; cell tests do not replace subsystem
randomized verification, VIP checks, CDC/RDC or physical qualification.
