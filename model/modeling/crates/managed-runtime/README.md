# Firmware-managed DMA runtime assembly

This crate connects a one-channel `pdma-model-dma` instance, the access window from
`pdma-model-hw-components`, and the async program from `pdma-model-firmware`.
It reuses the generic runtime and pipeline executors. The direct DMA transport in
`pdma-model-host::runtime` remains independent and has no firmware dependency.

## Ownership

| Source | Owns |
| --- | --- |
| `src/lib.rs` | Hardware/CPU construction and separate Host capability view. |
| `src/bus.rs` | Async transport to the hardware's work and management access methods. |
| `src/transport.rs` | Exclusive hardware access, notifications and worker wakeup. |
| `src/interrupt.rs` | Platform-pending notifications for independent software domains. |
| `src/run.rs` | Scoped driving and failure propagation for hardware and firmware. |

The access-window object enforces permissions. Firmware alone performs the
initialization write. Neither policy is reimplemented by runtime or scenario code.
`HostBus` cannot reveal the private management port or underlying DMA object.
Host and firmware have distinct `SoftwareCpu`/`InterruptRuntime` instances.

`Managed::new(memory)` returns `(system, host)` without starting actors or allocating
application memory. `host.bus` supplies work/readiness MMIO; `host.runtime` runs the
Host program and its ISR registration. `system.run()` drives firmware and the
persistent DMA graph. `system.run_until(host_execution)` ends simulation when the
supplied Host execution finishes, with actor errors taking priority.

All actor futures are scoped, not detached tasks. Dropping the run future stops
simulation and releases its actors; this is not an architectural reset or drain
acknowledgement. Firmware opens the window and then waits indefinitely without
busy polling. The MVP firmware interrupt input is idle; future management events
are outside this slice.

For exact register addresses, scope and acceptance, see the
[architecture MVP](../../docs/firmware-dma-mvp.md). Run the system surface with:

```sh
cargo test -p pdma-model-scenarios --test firmware_managed -- --nocapture
```
