# PDMA shared software

`pdma-model-software` supplies software ring algorithms shared by Host and firmware.
It depends on the PDMA contract and the generic runtime/schema building blocks,
with no dependency on host, firmware or DMA.

The public `ring` module owns `RingConfig<Entry>`, `SoftRing<Entry>`, `SoftRingIo`,
`MemoryRingIo`, `RegisterRingIo`, `RingCursorRegisters` and `SoftRingError`.
`RingConfig` validates software-selected storage geometry without storing cursors;
host reexports it as `host::RingConfig`. The `Ring`, `RingPointers` and `RingError`
definitions and PDMA pointer rules belong to `pdma-model-contract`.

`SoftRing` supplies CPU-owned `try_push`, `resume_push`, `peek` and `consume`.
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
