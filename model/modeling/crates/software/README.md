# PDMA shared software

`pdma-model-software` supplies PDMA software capabilities shared by Host and firmware.
It depends on the PDMA contract and the generic runtime/schema building blocks,
with no normal dependency on host, firmware or DMA. Tests use DMA as a behavioral oracle.

`dma::ChannelControl` provides CPU-owned discovery, faults, IRQ control and reset
over a borrowed bus without owning rings. Firmware release uses that same reset
handshake as the queue-owning `dma::Channel` driver. Host reexports the driver and
its error as `host::DmaError`; management/boot errors remain in Host.

`control::ControlQueueLayout` validates descriptor cursor addresses and supplies
memory cursor adapters. `control::InterruptEvent` supplies the processing task's
wait and a `ControlIsr` for its own program to register. The ISR wakes the task;
neither the event nor the layout implements resource policy or runtime dispatch.

The public `ring` module owns `RingConfig<Entry>`, `SoftRing<Entry>`, `SoftRingIo`,
`MemoryRingIo`, `RegisterRingIo`, `RingCursorRegisters` and `SoftRingError`.
`RingConfig` validates software-selected storage geometry without storing cursors;
host reexports it as `host::RingConfig`. The `Ring`, `RingPointers` and `RingError`
definitions and PDMA pointer rules belong to `pdma-model-contract`.

`SoftRing` supplies CPU-owned `try_push`, `resume_push`, `peek`, `consume` and
`is_empty`. The empty query reads validated published cursors without entry access
or changes to retained operations; it need not agree with a cached entry or an
unpublished push. Snapshot occupancy validation is the contract's
`Ring::validate_pointers`, separate from geometry and pointer movement checks.
Each operation borrows cursor I/O: `RegisterRingIo` uses register cursors for
ordinary Host DMA rings; `MemoryRingIo` accesses independent four-byte little-endian
cursors for both control endpoints. Callers provide entry memory and retain ownership
of its backing. The endpoint retains interrupted publication and decoded entries
awaiting consumption, without an authoritative cursor cache. Keep the ring and
transport stable across retained operations.

Encoding precedes producer publication; decoding precedes consumer publication.
`peek` allows protocol validation before a slot is released. Retried cursor writes
reconcile publication that already committed. Management request correlation,
resource allocation and reset policy belong to the caller.

An `Entry` parameter does not make PDMA ring protocol a framework responsibility.
Generic schema/runtime expose no ring protocol or ring types and have no normal
dependency on PDMA crates. Runtime provides transport, CPU ownership and generic
progress notifications. See the [control-channel contract](../../docs/control-channel.md)
for ordering, cancellation and memory lifetime requirements.

Run from `model/modeling`:

```sh
cargo test -p pdma-model-software --test soft_ring
```
