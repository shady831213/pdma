# Firmware control channel

One trusted PF owns the Host-to-firmware command and firmware-to-Host
response rings. VF software cannot access their control registers. Their geometry, producer/consumer cursors and entries live in memory.
They are not DMA task rings. Both directions use management ISRs; there is no event ring.

## Ownership and startup

- Contract defines a 24-byte request and 16-byte response, a 24-byte `ControlRingState`
  and a 48-byte `ControlQueue`. All have eight-byte alignment and little-endian encoding.
- PF Host allocates the descriptor and both entry buffers. It initializes the descriptor,
  publishes its address through bootstrap, enables the channel and rings the request doorbell.
- Firmware reads and validates the descriptor once, retaining accepted geometry.
  Subsequent operations access the independent cursor fields, not mutable geometry.
- Hardware owns only bootstrap, enable and two doorbells. It has no memory capability,
  resource-allocation policy, queue arithmetic or software handlers.
- Runtime connects each doorbell pulse to its destination's pending interrupt source.
  Each software program registers its own ISR, which wakes the ring-processing task.
  The ordinary direct-DMA assembly remains independent.

Firmware starts on subsystem reset release, registers its management ISR, discovers
the hardware channel count, closes every channel's work access and publishes
Ready. Host waits for Ready before setting up queues. Ready means firmware can serve
requests, not that Host can access DMA.

The managed harness supplies firmware with a register port bound by
`SoftwareCpu::bind_registers`. Runtime checks CPU ownership before every underlying
access poll, including resumed accesses. Firmware main performs ordinary register
operations and ISR registration; it does not capture CPU identity or wrap its
business flow in CPU scopes. Ordinary shared drivers impose no additional CPU
affinity: register capability checks, where explicitly configured, remain authoritative.

The always-on `ControlRegisters` map has a platform-selected base and contains bootstrap at offset 0 (eight bytes), enable at 8, request doorbell at 12,
and response doorbell at 16, for an extent of 20 bytes.
The platform-selected PF owns bootstrap/enable/request writes; firmware owns response writes. Wrong-owner
writes are rejected even if the value would not change. Doorbells are write-only:
one emits a coalescing pulse, zero has no effect, and hardware does not retain a one
in register storage. PF and firmware may read bootstrap/enable; VF accesses are rejected.

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

The firmware-local control endpoint owns attachment, receive/consume, response
publication and notifications. Resources serves ordinary requests serially:
check response space, receive one request, await its resource operation, publish
the response. A reset holds up later management requests, not independently driven
hardware or peer DMA. Host can still submit bounded asynchronous requests and
correlate responses by ID. Full response storage stops command admission; the
single firmware producer cannot lose the slot checked before starting an operation.

Notification is a hint, not a count or payload. Endpoints check the shared ring,
then wait if they cannot progress. A notification received before the wait is retained;
repeated notifications coalesce. Spurious or retried doorbells are harmless. The
two destinations do not share DMA interrupt causes, masks or handlers.

The existing coalescing doorbell pulse becomes a platform-pending interrupt.
`InterruptSource::claim` consumes it before dispatch; this slice adds no management
cause/mask/ACK registers. Host dispatch distinguishes Control and DMA vectors.
Firmware has its own control source, CPU and `InterruptRuntime`.
`software::control::InterruptEvent` retains the ISR wake hint across an early
interrupt or a cancelled wait. Clones share that hint, not a broadcast stream;
each endpoint has one processing waiter. A missing ISR is a dispatcher error,
never a fallback to direct bus notification.

The current memory backend completes accesses synchronously and serializes shared
storage. This is not a claim of PCIe cache coherence: a future backend/transport
must provide the required release/acquire ordering and visibility before doorbells.

## Requests and lifecycle

The managed platform supports independent Function-bound Host ports and an instance-configured
number of DMA channels. See [Function isolation](function-isolation.md) for identity,
ownership, interrupt routing and the remaining shared-memory trust boundary.
Every request names a target Function separately from its request ID and channel.
Firmware validates that target before any resource mutation; invalid targets return
InvalidFunction. Reset/release also require that target to own the channel, or return
PermissionDenied. Only PF submits these requests; the immutable MMIO requester
identity still governs PF/VF data access.
`AcquireChannel` selects the lowest free channel, opens its data window and
returns its identifier. NoResources means all channels are allocated.

`ResetChannel` closes the allocated channel's data window, awaits the hardware
reset handshake and reopens the now-disabled, unconfigured data block. Allocation
is retained. Host must retire its old driver and configure fresh rings before use.
`ReleaseChannel` closes the window, awaits the same reset and marks the channel
free without reopening it. Both reject out-of-range and unallocated identifiers
with InvalidChannel and NotAllocated. A queued duplicate release is processed
after its predecessor and observes NotAllocated; there is no Releasing scan or
Busy result.

Resource methods accept channel identity, not control request IDs or ring cursors.
The serve loop translates request/response records; reset and allocation operations
remain independent of that transport. Firmware is the only lifecycle issuer in
managed mode. Direct-DMA software retains privileged register access through the
same shared `software::dma::ChannelControl` handshake.

The DMA schema groups read-only discovery, one contiguous DmaAdmin block containing
global control and channel reset registers, then page-aligned channel data blocks.
Each data block owns 4096 bytes including reserved space; base and stride are
derived by schema placement. Software discovers the channel count before calculating
data addresses; hardware caches instance geometry during construction. Discovery remains
readable before firmware boot and regardless of grants. Host cannot read or write
global control or channel-admin blocks; granting any data window does not grant
administration. Each data block has its own gate. Cross-aperture transactions are
rejected even when adjacent gates are open; leaf read-only rules still apply.

`ChannelWindowRegisters` repeats compact twelve-byte permission blocks: open
at offset zero, firmware-only Function owner at four and readiness at eight. Contract provides only relative
geometry. `software::control::ManagementAddresses` carries platform-selected DMA,
permission-bank, boot and control-channel bases to software. The scenario harness
places these blocks, checks alignment/overflow/overlap, and performs bus address
translation. Hardware readiness derives from each gate, not a shadow allocation
table.

Closing MMIO alone is not a drain. PF must coordinate with the owning Function to stop ring/ISR use before reset or
release and retain memory until completion. Reset does not undo completed external
writes. Platform-pending DMA notifications remain associated with their original
Host. Runtime implements no allocation/reset policy.

### Future FLR ingress (not implemented)

A PCIe FLR notification may trigger firmware's Function-to-channels cleanup without
using the Host control ring. The lifecycle operations can be reused; receiving and
acknowledging FLR remains separate from control-ring request/response transport.
A future coordinator must serialize conflicting scopes, let independent Function
operations progress, and never depend on Host response-ring consumption to finish
FLR. The current serial control service is not that multi-ingress coordinator.
Application cleanup, ATS/transaction drain, DWC active/done, controller release
and later software readiness are distinct boundaries. Firmware and its management
access must remain outside the reset VF scope. No FLR, PCIe Function provisioning, PASID or
generic event merger is introduced here.

Malformed opcodes, invalid pointers and memory/bus failures stay typed errors.
The supervisor reports firmware execution failures; partial side effects are not
rolled back or automatically replayed. Recovery of a failed subsystem is not added.

## Shared software ring and cancellation

`pdma-model-contract` defines `Ring`, `RingPointers` and `RingError` as PDMA
protocol rules. The shared [PDMA software crate](../crates/software/README.md) owns
`ring::RingConfig<Entry>` and `ring::SoftRing<Entry>`; `host::RingConfig` reexports
the configuration type. `SoftRing` adds asynchronous push/peek/consume using
borrowed cursor I/O. `RegisterRingIo` supports ordinary Host DMA rings;
`MemoryRingIo` supports both control endpoints. Memory ownership stays with callers.
Neither Ring nor SoftRing knows management request IDs or allocation policy.
Generic schema/runtime expose no ring protocol or ring types. Runtime supplies
transport and independent progress notifications to the software crate; CPU-local
resources and main/ISR execution remain separate runtime services.

`Management::setup(bus, memory, ManagementConfig)` accepts platform `ManagementAddresses`, `ManagementRings` and
the processing task's `InterruptEvent`. Host registers `event.handler()` before setup.
`software::control::ControlQueueLayout` supplies the shared cursor adapters after
validating their addresses; initialization and attachment remain domain-specific.
`submit_acquire(function)`, `submit_reset(function, channel)` and
`submit_release(function, channel)` publish a request and notify firmware,
returning `ManagementRequestId` without waiting for execution. `try_response`
collects an available result, or returns None without waiting for firmware;
`next_response` waits on ISR hints until a result is available. Notification MMIO
itself may still suspend. The application correlates `response.id` with `id.raw()`.
No hidden background task drains responses and the management ISR only wakes the
software program. One client owns both cursors; mutable operations require exclusive
Rust borrowing, not a fixed modeled CPU identity. Construction and pending-state
queries need no active CPU. Migrating execution does not duplicate publications or
discard retained responses, and cancellation obligations remain unchanged.

The outstanding ledger is bounded by command-ring capacity, including requests
whose commands firmware has consumed but whose responses Host has not collected.
Full means software must collect results before submitting more. Response-ring
capacity is independent. Unknown/duplicate IDs or a reset/release result naming the wrong
channel are protocol errors; the offending response is not consumed.

Cancelled submission retains the same ID, target Function and publication state. Call
`resume_submission` before another submit/collection; it finishes publication and
notification without duplicating the command. Cancelling response collection keeps
the request outstanding; retry `try_response` or `next_response`. The serial
`acquire_channel`/`reset_channel`/`release_channel` conveniences require an idle client and use this
same path; `wait_pending` resumes an interrupted submission and collects a result.
Cancellation never retracts a request or releases a channel.
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

Acceptance covers pipelined acquisition, correlated management completions,
independent access grants, peer DMA during release, actual IRQ-driven DMA after allocation, release/reset/reacquisition,
full-ring backpressure, dormant firmware awakened through doorbells, malformed memory,
retained operations across cancellation and independent notification destinations.
C11 consumers verify exported descriptor bytes and bootstrap-register definitions.
`pdma_common.h` contains shared wire records; `pdma_host.h` and `pdma_firmware.h`
project the same register schemas into their visible access rights without changing
offsets. The hardware still enforces dynamic work-window permissions.
