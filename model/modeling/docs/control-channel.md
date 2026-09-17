# Firmware control channel

The managed assembly has independent Host-to-firmware command and
firmware-to-Host response rings. These are not DMA task rings. There is no event
ring, mailbox payload, management interrupt, or direct Host/firmware function call.
Both endpoints currently poll; runtime only supplies memory and register access.

## Ownership and startup

- `contract::ControlRequest` and `ControlResponse` define the 16-byte,
  eight-byte-aligned, little-endian records. Both carry a 64-bit request id.
- `hw-components::ControlChannel` owns the register state and write permissions.
  It has no memory capability and does not interpret management requests.
- Host owns control-ring allocation and publishes command records. Firmware
  reads them through its injected memory interface and publishes responses.
- Firmware owns allocation policy. The first slice manages one physical DMA
  channel, zero, for one Host. It does not add Function identity or ownership tables
  to DMA or runtime. The ordinary direct-DMA assembly remains independent.

Firmware starts on subsystem reset release, closes DMA work access and publishes
Ready. Ready means that firmware can service the control channel; it does not open
DMA access. Host waits for Ready, allocates both control rings, configures their
geometry and enables the control channel. It then requests a DMA channel. Software
CPU and interrupt-dispatch ownership rules remain unchanged.

`CONTROL_CHANNEL_BASE` is `0xffff_fe00`. `ControlRegisters` composes command ring
registers at offset 0, response ring registers at 24, and enable at 44, with extent
48. Each ring contains base, capacity, producer and consumer. Both domains can
read; Host alone configures geometry and activation. Host writes command producer
and response consumer; firmware writes command consumer and response producer.
Hardware rejects writes by the other domain even when the value would not change.

Activation validates both ring geometries atomically using the existing `Ring`
trait. Capacity is a nonzero power of two below 2^31. Cursors are wrapping u32
entry counts. Hardware rejects overflow/over-consumption; it does not check
software memory allocation or disjointness of ring buffers. Configuration changes
and disabling are rejected after activation. There is no live control-ring reset
in this slice; keep storage valid until the subsystem is safely stopped/reset.

## Requests and lifecycle

`AcquireChannel` ignores the request's channel field. Firmware selects channel zero
if available, opens its work window, and returns Success with that channel id.
An existing allocation returns NoResources without changing permissions. Host then
allocates/configures the ordinary DMA rings and uses the existing channel driver;
firmware does not proxy DMA descriptors or responses.

`ReleaseChannel` returns InvalidChannel for an index other than zero or
NotAllocated for a channel not currently allocated. For a valid release firmware:

1. Closes Host work MMIO without disabling control-channel or firmware access.
2. Requests the existing DMA channel reset through its privileged MMIO view.
3. Polls the actual busy register while the unchanged DMA pipeline drains old work.
4. Frees the allocation and publishes Success only after reset completes.

Closing MMIO alone is not a drain. Release success means old accepted work can no
longer access command, SG, payload or response memory. External memory already
written is not rolled back. Host must stop its ring/ISR use before release and
retain application memory through completion. A new grant needs a fresh channel
driver/setup. Already platform-pending DMA notifications still belong to their
original Host; this slice does not reassign resources to another Function or IRQ
destination. External bus drain and PCIe lifecycle release are not modeled here.

Firmware checks response capacity before accepting another command, so a full
response ring backpressures management side effects. Record storage precedes
producer publication, and successful decode precedes consumer publication. The
common `Ring` calculations implement bounds and wrap rules at both endpoints.
Invalid opcodes are codec errors, not allocation requests. Control-memory,
register or codec failures remain typed firmware execution errors surfaced by
the scoped supervisor; partial management side effects are not rolled back or
automatically retried. Recovery of that failed subsystem is outside this slice.

## Host API and cancellation

`Management::setup(bus, memory, ManagementRings)` configures control storage.
`acquire_channel` and `release_channel` submit requests and await correlated
responses. The Host client allows one outstanding request; firmware also processes
requests serially. Ring capacity is independently configurable, not fixed to one.

The client stores its pending request before publication. Cancelling its future
or imposing a software timeout does not retract accepted work. Keep the client
and storage alive and call `wait_pending` to resume that same operation. Another
submission returns ManagementBusy until the previous response is collected.
`wait_pending` returns the raw typed management response, including failure status.

On resumption the client reads the authoritative producer/consumer registers to
reconcile a write that committed before cancellation. It caches a decoded response
before acknowledging its slot, so a reused slot cannot replace the retained result.
Mismatched request ids are rejected without consuming the response. The supported
transport uses scoped MMIO futures with no detached post-cancellation writes.
Dropping the whole client is not a resource release or control-channel reset.

## Verification

Run from `model/modeling`:

```sh
cargo test -p pdma-model-contract --test control
cargo test -p pdma-model-hw-components --test control_channel
cargo test -p pdma-model-scenarios --test firmware_managed
cargo test -p pdma-model-contract --features export-c-header --test header_c
```

Scenarios cover actual IRQ-driven copy after acquisition, release/reacquisition,
release during a multi-chunk copy, and cancellation at both pointer-publication
boundaries. Hardware tests cover access ownership, malformed geometry, locking,
queue bounds and u32 wrap. Generated C records/register definitions share the
same schema as Rust and are checked by a compiled C11 consumer.
