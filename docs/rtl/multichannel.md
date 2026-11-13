# Shared execution with one to eight channels

Status: functional and structural checks pass at this checkpoint; mapping and
aggregate coverage remain pending. See the [validation record](../../verification/dma/results.md#2026-11-13-multichannel-checkpoint). The accepted scope
is every integer `CHANNEL_COUNT` from 1 through 8. Develop and synthesize first
at 2, preserve the 1-channel case, check intermediate configurations, and close the
complete functional/throughput/CDC/RDC/synthesis loop at 8 before completion.
Copy/Fence and the existing fault/error/reset semantics remain the command scope.

## Resources and ownership

| Resource | Owner and capacity | Release |
| --- | --- | --- |
| Accepted descriptor and command progress | One retained context per channel | Terminal command handling or completed reset cleanup |
| Retained burst metadata and read data | Eight 64-byte rows reserved per channel | Successful write retirement or safe cancellation |
| Data RAM | One shared 1R1W provider, `8 * CHANNEL_COUNT` rows of 512 bits | Row validity follows its channel/slot owner; contents need no reset |
| Issue, decode and payload preparation | One shared execution datapath | Command completion, safe park, or closing the selected owner |
| Requests and receipts across domains | Shared bounded queues, explicit channel/slot tags | Handoff transfers custody; a request terminates only on its own receipt |
| Bus outstanding metadata and data | Shared read/write endpoints | Existing AR/R and AW/W/B completion rules |

The reserved per-channel space implements the model's retained-state guarantee.
It avoids a parked owner consuming a global pool needed by a healthy peer. The
model owns one bounded chunk payload per accepted channel; RTL may retain several
bursts while preserving the same ownership, replay and progress contract. It does
not require the model to acquire eight physical slots or reproduce RAM cycles.
At eight channels the logical retained-data capacity is 4KiB, independently of
the shared FIFO/outstanding capacities. There are no per-channel engine replicas.

Ordinary command and chunk computation uses the selected context. State banks
also consume tagged receipts for closing owners and acknowledge independent reset;
these are resource-owner updates, not additional command-execution pipelines.
Prepared data has explicit channel ownership. A reset of A cannot invalidate B's
prepared write, outstanding receipt or notification.

## Scheduling and lifecycle

Round-robin admission uses the existing common-cell selector. Eligibility excludes
reset, stopped, fatal and faulted channels. Cursor wrap uses the actual count,
including non-powers of two. A healthy command retains its grant across chunks;
the channel consumer advances once at valid descriptor acceptance. Resuming a
parked command reacquires a grant and uses its stored descriptor/progress/data.

Closing an owner immediately blocks its new requests. Requests already accepted
by the core-to-bus queue remain obligations even before AXI handshakes. Return
tags identify the channel and burst/control slot until the core consumes them.
A new reset request has priority over an old cleanup acknowledgement at the
same core edge. In particular, a global request coinciding with a channel's old
clear retains that channel's closing state for the new collector; the global
operation cannot lose its acknowledgement and wait indefinitely.
Only that channel's outstanding work gates its clear; unrelated pending work does
not gate an idle/parked channel's reset. Shared queues are never reset or flushed
by a channel reset. DMA reset collects every channel's cleanup acknowledgement.

The ordinary bus profile retains ID-zero ordering separately for reads and writes.
Channel identity is independent of AXI ID. Shared queue/endpoint backpressure and
external ordering can delay peers; no independent external response-ordering or
zero-interference guarantee is claimed. A parked channel must nevertheless leave
healthy peers a usable retained-data reservation and shared execution opportunity.

## Integration boundary

Memory address sideband uses 35 bits: `ARUSER/AWUSER[34:32]` identifies
the channel, and `[31:0]` retains the full context selector. This fixed maximum
tag width is independent of the instantiated count; eligibility, address bounds
and round-robin wrap use the actual count. The channel selector is not a Function/PASID or a platform fault tag.
The instance count is immutable and reported by discovery; descriptor and register
semantics remain unchanged. Channel notifications have independent pending/accept
state, separate from the DMA-wide fatal notification. Platform routing, firmware
and CPU implementation remain outside this DMA-only slice.

`CommandState::Context` is a bank of channel-owned registers. The selected row
feeds one decode/address datapath; tagged terminal metadata returns can independently
discharge a closing row. `CopyWindow` uses a flat `channel * 8 + slot` row address;
source/destination advancement and retired-prefix arithmetic are shared. Its
single prepared-write cache is tagged by channel; invalidating a cache never
discards the authoritative retained RAM bytes.

All final results must identify the actual channel configuration. The implementation
profile resolves parameters, IRQ ports and RAM geometry before launching tools;
its frozen dictionary is part of the source/configuration identity. The same source
snapshot can elaborate different counts, but its qualification is configuration-specific.

## Crossing review boundaries

The native source-bound gates for the current 2-channel and 8-channel elaborations
are recorded separately in `impl/dma/reviews/copy.tcl` and `copy-8.tcl`. They retain
raw findings and require every exact native endpoint to match before applying a
review. A changed source/configuration cannot inherit approval by a finding count.

Both configurations retain four registered Gray-pointer decode convergences and
three independent local-credit convergences, with the same bounded crossing and
local synchronizer timing obligations described in [the crossing analysis](throughput-review.md#cdc--rdc-归因与物理约束).
Resetless chunk metadata and RAM/cache data have reset validity/occupancy guards;
allocation or a qualified memory read establishes contents before any consumer.
Reset closes issue, retains all accepted debt, and invalidates only the cleared
owner. Cold reset still covers all three domains, with local synchronous release.

RDC dead-sink groups name exact unused FIFO data-match inputs (`MATCH_COUNT_WIDTH=0`)
and the unused RAM collision result (`FORWARD_NEW_DATA=false`). At two channels,
one additional named `eligible` intermediate is unconnected in the native graph:
its flat fanout and endpoint fanout are both zero, while live `choice` and `selected`
nodes have consumers. Only that precise two-channel node is covered. This is not
an exception for live arbitration logic or an inference that all resetless storage
is safe. Group counts check coverage of these proven endpoints, not correctness.

The reset request/old-clear priority fix was checked against the complete native
finding fields before retaining these source-class reasons. Fresh gates bind to
the new source identity; native negative cases reject stale identity, wrong or
missing endpoints, an omitted finding, invalid grouping and incorrect counts.
Physical crossing delay/skew, synchronizer MTBF and reset recovery/removal remain
separate obligations; the structural gates alone are not physical signoff.
