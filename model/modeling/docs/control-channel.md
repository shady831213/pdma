# Firmware control channel

The managed assembly has independent Host-to-firmware command and firmware-to-Host
response rings. Their geometry, producer/consumer cursors and entries live in memory.
They are not DMA task rings. Both directions use management ISRs; there is no event ring.

## Ownership and startup

- Contract defines 16-byte request/response records, a 24-byte `ControlRingState`
  and a 48-byte `ControlQueue`. All have eight-byte alignment and little-endian encoding.
- Host allocates the descriptor and both entry buffers. It initializes the descriptor,
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
business flow in CPU scopes. Shared driver internals retain their CPU-local guards.

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

The firmware-local control endpoint owns attachment, nonblocking receive, response
publication and notifications. One software service owns both ring cursors. It
samples pending channel resets, accepts new requests and publishes available
results. A release waiting on hardware does not block unrelated requests.
Responses can therefore complete out of submission order; their IDs are authoritative.
When response storage is full, firmware stops accepting commands and retains
release state until Host returns space. It does not reserve a response slot for
the lifetime of each reset or grow an unbounded response backlog.

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

This slice has one trusted Host and an instance-configured number of DMA channels.
`AcquireChannel` ignores the request's channel field. Firmware selects the lowest
free channel, opens only its work window and returns Success with that identifier.
If every channel is allocated or releasing it returns NoResources. Host then
allocates/configures ordinary DMA rings; firmware does not proxy DMA task descriptors.

`ReleaseChannel` returns InvalidChannel for an out-of-range identifier, NotAllocated
for a free channel, or Busy for an already releasing channel. A valid release
closes only that channel's Host MMIO and requests its existing DMA reset. Firmware
retains the release request while servicing other channels. It publishes Success
and makes the channel available again only after hardware clears reset busy and
response storage is available. Runtime does not implement this policy.
Both software domains use `software::dma::ChannelControl` for this register-only
reset handshake. It does not construct or own DMA rings.

The firmware resource states are Free, Allocated and Releasing(request). There is
one entry per hardware channel, not one async task/executor per channel.
`ChannelControl::reset_in_progress` samples the same register used by the blocking
reset helper; it neither requests reset nor fabricates completion. While resets
are pending the firmware service cooperatively polls them. Otherwise it sleeps
on its ISR event, including when response backpressure prevents progress.

`hw-components::ChannelWindows` composes existing access windows over each DMA
channel register block. Firmware controls channel N at `0xff00_0000 + 16*N`;
Host reads its readiness at `0xff00_0008 + 16*N`. `ChannelId` supplies checked
addresses, and `ChannelWindowRegisters` exports the same repeated geometry.
Readiness is derived from each gate, never a shadow allocation table.
Cross-channel transactions are rejected even when both gates are open. Shared
device registers are accessible while any channel is granted, preserving the
single-channel case. A trusted Host must coordinate device-wide reset/IRQ settings:
this is not isolation between mutually untrusted Functions or virtual machines.

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

`Management::setup(bus, memory, ManagementConfig)` accepts `ManagementRings` and
the processing task's `InterruptEvent`. Host registers `event.handler()` before setup.
`software::control::ControlQueueLayout` supplies the shared cursor adapters after
validating their addresses; initialization and attachment remain domain-specific.
`submit_acquire` and `submit_release` publish a request and notify firmware,
returning `ManagementRequestId` without waiting for execution. `try_response`
collects an available result, or returns None without waiting for firmware;
`next_response` waits on ISR hints until a result is available. Notification MMIO
itself may still suspend. The application correlates `response.id` with `id.raw()`.
No hidden background task drains responses and the management ISR only wakes the
software program. One CPU-local client owns both cursors.

The outstanding ledger is bounded by command-ring capacity, including requests
whose commands firmware has consumed but whose responses Host has not collected.
Full means software must collect results before submitting more. Response-ring
capacity is independent. Unknown/duplicate IDs or a release result naming the wrong
channel are protocol errors; the offending response is not consumed.

Cancelled submission retains the same ID and publication state. Call
`resume_submission` before another submit/collection; it finishes publication and
notification without duplicating the command. Cancelling response collection keeps
the request outstanding; retry `try_response` or `next_response`. The serial
`acquire_channel`/`release_channel` conveniences require an idle client and use this
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

Acceptance covers pipelined acquisition, out-of-order management completions,
independent access grants, peer DMA during release, actual IRQ-driven DMA after allocation, release/reset/reacquisition,
full-ring backpressure, dormant firmware awakened through doorbells, malformed memory,
retained operations across cancellation and independent notification destinations.
C11 consumers verify exported descriptor bytes and bootstrap-register definitions.
`pdma_common.h` contains shared wire records; `pdma_host.h` and `pdma_firmware.h`
project the same register schemas into their visible access rights without changing
offsets. The hardware still enforces dynamic work-window permissions.
