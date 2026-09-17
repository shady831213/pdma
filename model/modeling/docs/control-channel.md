# Firmware control channel

The managed assembly has independent Host-to-firmware command and firmware-to-Host
response rings. Their geometry, producer/consumer cursors and entries live in memory.
They are not DMA task rings. There is no event ring or management ISR.

## Ownership and startup

- Contract defines 16-byte request/response records, a 24-byte `ControlRingState`
  and a 48-byte `ControlQueue`. All have eight-byte alignment and little-endian encoding.
- Host allocates the descriptor and both entry buffers. It initializes the descriptor,
  publishes its address through bootstrap, enables the channel and rings the request doorbell.
- Firmware reads and validates the descriptor once, retaining accepted geometry.
  Subsequent operations access the independent cursor fields, not mutable geometry.
- Hardware owns only bootstrap, enable and two doorbells. It has no memory capability,
  resource-allocation policy, queue arithmetic or software handlers.
- Runtime connects each doorbell pulse to its destination's generic notification waiter.
  The ordinary direct-DMA assembly remains independent.

Firmware starts on subsystem reset release, closes DMA work access and publishes
Ready. Host waits for Ready before setting up queues. Ready means firmware can serve
requests, not that Host can access DMA.

The always-on `ControlRegisters` map at `CONTROL_CHANNEL_BASE` (`0xffff_fe00`)
contains bootstrap at offset 0 (eight bytes), enable at 8, request doorbell at 12,
and response doorbell at 16, for an extent of 20 bytes.
Host owns bootstrap/enable/request writes; firmware owns response writes. Wrong-owner
writes are rejected even if the value would not change. Doorbells are write-only:
one emits a coalescing pulse, zero has no effect, and hardware does not retain a one
in register storage. Both domains may read bootstrap/enable.

Enable freezes the bootstrap address and rejects disabling. Geometry validation now
belongs to software, not the MMIO block. `Ring` requires a nonzero power-of-two
capacity below 2^31 and validates aligned, representable storage extents. Cursors
are wrapping u32 entry counts. `SoftRing` validates observed pending counts.
Host owns command producer and response consumer; firmware owns the opposite
cursors. These in-memory ownership rules are a software protocol, not hardware
write protection. Host must keep accepted geometry and firmware-owned cursors intact.

There is no live control-channel rebind/reset in this slice. Repeated Host setup
rejects an already enabled channel before touching its descriptor. Keep storage
valid until the subsystem is safely stopped/reset. Concurrent configuration by
multiple Host clients is not supported.

## Notification and ordering

Each endpoint stores the entry before publishing its producer. It decodes an entry
before publishing its consumer. Cursor updates write exactly the corresponding
four-byte field; there is no whole-descriptor read/modify/write.

Host rings the request doorbell after command publication and response consumption.
Firmware rings the response doorbell after command consumption and response
publication. Space-return notifications let a peer blocked by a full ring resume.
Firmware checks response space before accepting another command.

Notification is a hint, not a count or payload. Endpoints check the shared ring,
then wait if they cannot progress. A notification received before the wait is retained;
repeated notifications coalesce. Spurious or retried doorbells are harmless. The
two destinations do not share DMA interrupt causes, masks or handlers.

The current memory backend completes accesses synchronously and serializes shared
storage. This is not a claim of PCIe cache coherence: a future backend/transport
must provide the required release/acquire ordering and visibility before doorbells.

## Requests and lifecycle

`AcquireChannel` ignores the request's channel field. Firmware selects channel zero
if available, opens its work window and returns Success with that identifier.
An existing allocation returns NoResources without changing permissions. Host then
allocates/configures ordinary DMA rings; firmware does not proxy DMA task descriptors.

`ReleaseChannel` returns InvalidChannel for another identifier or NotAllocated
for an unallocated channel. A valid release closes Host work MMIO, requests the
existing DMA channel reset, waits for its actual busy register to clear, then frees
the allocation and publishes Success. Runtime does not implement this policy.

Closing MMIO alone is not a drain. Host must stop its ring/ISR use before release
and retain memory until completion. Reset does not undo completed external writes.
A new grant needs a fresh channel driver/setup. Already platform-pending DMA
notifications remain associated with their original Host. Function reassignment,
external bus drain and PCIe lifecycle release remain outside this slice.

Malformed opcodes, invalid pointers and memory/bus failures stay typed errors.
The supervisor reports firmware execution failures; partial side effects are not
rolled back or automatically replayed. Recovery of a failed subsystem is not added.

## Shared software ring and cancellation

`pdma-model-contract` defines `Ring`, `RingPointers` and `RingError` as PDMA
protocol rules. The shared [PDMA software crate](../crates/software/README.md) owns
`ring::RingConfig<Entry>` and `ring::SoftRing<Entry>`; `host::RingConfig` reexports
the configuration type. `SoftRing` adds CPU-owned asynchronous push/peek/consume using
borrowed cursor I/O. `RegisterRingIo` supports ordinary Host DMA rings;
`MemoryRingIo` supports both control endpoints. Memory ownership stays with callers.
Neither Ring nor SoftRing knows management request IDs or allocation policy.
Generic schema/runtime expose no ring protocol or ring types. Runtime supplies
transport, CPU ownership and independent progress notifications to the software crate.

`Management::setup(bus, memory, ManagementRings)` accepts the descriptor address
and entry configurations. The client allows one outstanding request; firmware
serves requests serially. Ring capacity is independently configurable.

Cancelling `acquire_channel` or `release_channel` does not retract a request.
Keep the client/storage alive and call `wait_pending` before submitting again.
The client retains request correlation and publication/consumption completion.
SoftRing retains an interrupted publication or decoded entry, reconciling committed
cursor writes on resumption. A mismatched response ID is not consumed. Once consumed,
the Host retains the result until its space-return notification completes, so reusing
the freed slot cannot replace that result. Doorbell retry never republishes the entry.

Dropping the client or firmware program is not a resource release or hardware reset.
The supported MMIO futures have no detached post-cancellation writes.

## Verification

Run from `model/modeling`:

```sh
cargo test -p pdma-model-software --test soft_ring
cargo test -p hardware-interface-runtime --test notifications
cargo test -p pdma-model-host --test management_memory
cargo test -p pdma-model-firmware
cargo test -p pdma-model-hw-components --test control_channel
cargo test -p pdma-model-scenarios --test firmware_managed
cargo test -p pdma-model-contract --features export-c-header --test header_c
```

Acceptance covers actual IRQ-driven DMA after allocation, release/reset/reacquisition,
full-ring backpressure, dormant firmware awakened through doorbells, malformed memory,
retained operations across cancellation and independent notification destinations.
C11 consumers verify exported descriptor bytes and bootstrap-register definitions.
