# PDMA Copy Engine RTL

For code-corresponding block diagrams, state machines and timing, see the
[implemented microarchitecture](../docs/rtl/microarchitecture.md) and
[performance report](../docs/rtl/performance.md).

This directory contains the in-progress core RTL for PDMA, a programmable copy
engine for virtualized systems. The managed resource-control and platform-integration layers remain
outside the first RTL phase; see the [project overview](../README.md).

The first implemented slice is the DMA register-access path: AXI4-Lite 32/64,
reg/core CDC and channel/ring register owners. DMA integration tests cover it alongside the standalone frontend test in `common_cell/tests`.
The [minimal Linear Copy and Fence slice](../docs/rtl/minimal-copy.md) now connects
this path to one execution owner and an ordinary AXI memory endpoint. Fence32/64
writes a software-observable memory marker after prior channel work. It is not a
complete DMA or a physically qualified design; the remaining command families,
unaligned/full-profile throughput and physical closure remain open. Configuration-specific
functional and implementation results are recorded in the
[verification checkpoints](../verification/dma/results.md). The retained burst window and asynchronous
queues are described in the slice document; measured results are in the
[throughput review](../docs/rtl/throughput-review.md).
The subsequent [organization review](../docs/rtl/organization-review.md) records
source ownership, package resource loading and flow/test cleanup.

The first RTL phase is **DMA only**, confirmed 2026-10-29. CPU implementation,
managed peripheral hardware, firmware execution and full subsystem integration
are deferred until DMA RTL converges. Keep external control/memory/notification
interfaces suitable for that integration; do not build those external owners now.

The [system specification](../model/modeling/docs/spec.md) defines observable
hardware, firmware, Host and platform behavior. The
[RTL architecture draft](../docs/rtl/architecture.md) records confirmed interfaces,
clock/reset requirements and decisions still needed before coding.
The [register-interface proposal](../docs/rtl/register-interface.md) develops the
32/64-bit access, submission and CDC rules; its implementation status separates
the tested register slice from outstanding lifecycle/datapath integration.
The related [memory interface](../docs/rtl/memory-interface.md),
[datapath](../docs/rtl/datapath.md) and [reset/CDC](../docs/rtl/reset-cdc.md) drafts
define transaction ownership and identify correctness constraints on pipelining.
The [1..8 channel shared engine](../docs/rtl/multichannel.md) has an eight-channel
qualification requirement. Its configurable composition is described in the
[configuration contract](../docs/rtl/configuration.md); later work is listed in the
[architecture plan](../docs/rtl/architecture.md#后续执行顺序).
The implementation target is 12nm/Genus without ULVT,
core/bus 1GHz and reg 500MHz, with separate VC SpyGlass CDC/RDC checks; see
[implementation checks](../impl/dma/README.md).
Synthesis now uses an explicit [RAM boundary budget](../docs/rtl/memory-budget.md)
by default, with ordinary RTL ungrouping and boundary optimization enabled.
The [qualification record](../docs/rtl/memory-budget-review.md) distinguishes
mapped logic from excluded SRAM cost; the generic array mode remains available.

## Source ownership

| Path | Contents |
| --- | --- |
| `src/dma/` | DMA channels, shared execution, register behavior and bus endpoints |
| `src/dma/control/` | Register owners, IRQ delivery and global reset collection |
| `src/dma/execution/` | Command owner and its retained burst window |
| `src/dma/memory/` | Typed memory port, CDC queues and DMA-to-bus attribute mapping |
| `src/hw_components/` | Deferred placeholder for managed peripheral hardware; outside the first RTL phase |
| `src/tests/` | Native Veryl block tests and their local support |
| `common_cell` dependency | Storage and transport primitives, bus endpoints, constraints and passive checkers from my-veryl-cells |
| `tech_cell_generic` dependency | Generic RAM providers from my-veryl-cells |

Keep DMA in one native project, with reusable cells in their own library project;
the Rust module tree is not a prescribed RTL hierarchy.
The DMA project consumes [my-veryl-cells](https://github.com/shady831213/my-veryl-cells)
through exact native package versions and a committed lockfile. Compiler recipes
come from the pinned `external/my-veryl` submodule; see [tool setup](../tools/rtl-flow/README.md).
Do not create empty pipeline controllers or one project per proposed module.
Firmware stays software under `model/modeling/crates/firmware`; a hardware wrapper
does not own its policy or introduce another software runtime.

Veryl is the design source. Generated SystemVerilog, dependency checkouts,
filelists, simulator caches and waveforms are build artifacts. Use native
Veryl source discovery and dependencies. The configured integration project selects
type arguments and references the real source directory; it never copies the
design or substitutes a hand-maintained source list for native discovery. `Veryl.lock` pins the resolved native project dependencies.

Regenerate layout metadata from the Rust contract, then run the native checks:

```sh
# Repository root; use --write after intentional contract changes.
bash tools/rtl-flow/layout.sh --check
cd rtl
bash ../tools/rtl-flow/veryl.sh fmt --check
bash ../tools/rtl-flow/veryl.sh build
VERYL_DUT_REUSE=0 bash ../tools/rtl-flow/veryl.sh test
VERYL_DUT_REUSE=0 bash ../tools/rtl-flow/veryl.sh test --backend cranelift
```

See [DMA register ownership and integration](src/dma/README.md) and
[test coverage](src/tests/README.md) before wiring another owner.

Cell unit tests stay with [my-veryl-cells](https://github.com/shady831213/my-veryl-cells).
PDMA's tests cover integration with the resolved cells. The [DMA module tree](src/dma/README.md)
shows containment within each owner group without adding hardware wrappers.

## Verification and implementation

Use native Veryl tests for local state transitions and interfaces. DMA-level
VCS/UVM/VIP verification belongs in [verification/dma](../verification/dma/README.md).
Design-specific implementation profiles and reviews belong in [impl/dma](../impl/dma/README.md);
generic tool adapters live in `tools/rtl-flow/implementation`. Reusable cell
constraints/checkers are declared by their Veryl package and travel in snapshots.
Model scenarios remain executable software/hardware semantic references, not
cycle-accurate golden traces.
Initial testbenches drive DMA ports directly and supply memory/fault/timer/IRQ
behavior. They do not require a CPU, synthesized firmware or a recreated managed subsystem.

[Tool integration](../tools/rtl-flow/README.md) independently maintains a fixed
Veryl version and patches, initially taken from a qualified
[pcie_modeling](https://github.com/shady831213/pcie_modeling) snapshot.
Its separate compiler smoke test is not a PDMA design test or EDA qualification.

## Static integration profiles

Use `bash rtl/flow/snapshot.sh NEW_OUTPUT [PROFILE_JSON]` for DMA-level simulation
and implementation. The default profile is `rtl/profiles/default.json`; an override
file supplies only independent inputs. `PDMA_CHANNEL_COUNT` may select a count
when creating the snapshot. Subsequent VIP and implementation runs consume that
snapshot's configuration; conflicting overrides fail before launching a tool.
The generic `tools/rtl-flow/snapshot.sh` remains available for native projects.
See [configuration and parameter dependencies](../docs/rtl/configuration.md).
