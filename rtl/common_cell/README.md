# Common hardware cells

This native Veryl library provides reusable transport, storage and selection
components. Interfaces depend on payload types, dimensions and clock/reset
contracts, not on a particular device protocol or software lifecycle.

## Source and changes

Imported sources and tests come from
[pcie_modeling at 67989b3e](https://github.com/shady831213/pcie_modeling/tree/67989b3ebbdc8466e79ded2c65ad64c0da5585b0/rtl/common_cell).
The matching [generic RAM providers](https://github.com/shady831213/pcie_modeling/tree/67989b3ebbdc8466e79ded2c65ad64c0da5585b0/rtl/tech_cell/generic)
live separately under `../tech_cell/generic`. `upstream.sha256` records the imported
files relative to `rtl`. The original cell/test source bodies are unchanged;
the project manifest now also declares optional package-owned flow resources.
`DuplexMailbox`, `AsyncQueue`, their checkers and tests were added during PDMA
development; this is source provenance, not a consumer dependency.
Updates must be deliberate, with provenance and regression results; there is no
automatic synchronization or dependency on an external checkout.
The instance-scoped constraints own no platform clocks, global clock groups,
external IO budgets or reset policy.
`constraints/ip.tcl` is the package entry declared in `Veryl.toml`. It loads the
cell contracts and exports tool-independent validation, native path descriptions
and synchronizer collection queries. Shared physical-object selection belongs
to `constraints/objects.tcl`, not to a particular CDC primitive.
The package also supplies `verification/cdc_custody.sv`, defining
`common_cell_mailbox_checker` and `common_cell_async_queue_checker`. Consumers
explicitly bind these passive, payload-typed checkers to their elaborated instances.
They check data, order, capacity and backpressure; they neither drive the hardware
nor implement a system reset/drain policy. They are not synthesized. Scenario-level
timeouts or explicit drain checks supply liveness/end-of-test completion checking.

## Boundaries and use

| Cells | Contract |
| --- | --- |
| `ReadyValidSlice` | Typed single-clock Bypass/Forward/Reverse/Full transport |
| `BoundedIndex`, `RoundRobin` | Pure bounded selection; ownership/cursor remain with caller |
| `Ram1RW`, `Ram1R1W`, `Ram1R1WAsync` | Typed RAM ports and provider contracts; optional same-clock forwarding |
| `DuplexMailbox` | Two typed acknowledged CDC mailboxes with independent release and bounded custody |
| `AsyncQueue` | Typed ready/valid wrapper over the standard Gray-pointer FIFO |

Slice modes have capacity/empty latency of 0/0 (Bypass), 1/1 (Forward), 1/0
(Reverse), and 2/1 (Full), with unstalled rate one beat per clock. Reset clears
custody, not a protocol obligation; invalid payload values are unspecified. The
surrounding protocol owner remains responsible for obligations that outlive a
transfer through the slice.

RAM providers have synchronous reads, no content reset and bit masks. An idle
output need not retain the last read value; a consumer holding a result under
backpressure must capture it. Dimensions and addresses must be valid.
Same-address read/write collisions are excluded
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
not supported. Cancellation of a higher-level operation does not by itself release
accepted transfers; the integrating owner must resolve its outstanding obligations.

`AsyncQueue #(TYPE, DEPTH)` has independent source/destination clocks, resets and
ready/valid ports. DEPTH must be a power of two and at least four. It uses the pinned
standard FIFO's registered Gray pointers and two synchronizer stages. The queue owns
ordered entries, not transaction lifecycle. Its cold-reset contract matches the
mailbox's; operation cancellation and outstanding-transaction cleanup remain outside
the queue.
`constraints/async_queue.tcl` bounds both pointer directions and stored-data paths;
preserve synchronizer stages and keep their local paths normally timed. Physical
skew/delay and MTBF qualification remain integration responsibilities.

For direct timing integration, source `constraints/ip.tcl` and invoke
`common_cell_cdc::duplex_mailbox` or `common_cell_cdc::async_queue` with an instance
path, the actual source/destination clock collections and a maximum-delay budget
in the active SDC units. Loading the entry defines functions only. Apply/check
those constraints separately from structural CDC review; no waiver is inherited.

Device-specific decoding, resource ownership, fault recovery and lifecycle policy
belong to consumers. Repeated application syntax alone does not justify adding a
component to this library.

## Verification

Use the integrating project's qualified Veryl compiler, including support for
generic module parameters. From this package's `tests` directory:

```sh
VERYL_DUT_REUSE=0 veryl test
VERYL_DUT_REUSE=0 veryl test --backend cranelift
```

The imported tests cover slice modes, custody/pressure, selection, RAM masks,
depths, forwarding and clock ownership. The mailbox test checks independent wide
payload types, retained snapshots, backpressure and no duplicate delivery.
The startup test covers both release orders and early custody with a reset peer.
Consumers own integration scenarios and bindings. Cell tests do not replace
system-level randomized verification, protocol VIP checks, CDC/RDC or physical qualification.
