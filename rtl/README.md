# PDMA Copy Engine RTL

This directory contains the in-progress core RTL for PDMA, a programmable copy
engine for virtualized systems. The managed resource-control and platform-integration layers remain
outside the first RTL phase; see the [project overview](../README.md).

The first implemented slice is the DMA register-access path: AXI4-Lite 32/64,
reg/core CDC and channel/ring register owners. Seven native tests exercise it.
The [minimal Linear Copy and Fence slice](../docs/rtl/minimal-copy.md) now connects
this path to one execution owner and an ordinary AXI memory endpoint. Fence32/64
writes a software-observable memory marker after prior channel work. It is not a
complete DMA or a physically qualified design; the remaining command families,
multichannel execution and throughput/physical closure still need implementation.

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
The [next implementation steps](../docs/rtl/architecture.md#后续执行顺序)
start with concrete transaction resources and reset drain, then a two-channel
functional slice. The existing evaluation environment remains the baseline;
future tool/technology migration does not block this work.

## Source ownership

| Path | Contents |
| --- | --- |
| `src/dma/` | DMA channels, shared execution, register behavior and bus endpoints |
| `src/hw_components/` | Deferred placeholder for managed peripheral hardware; outside the first RTL phase |
| `src/tests/` | Native Veryl block tests and their local support |
| `common_cell/` | Protocol-independent reusable cells and separate native tests |
| `tech_cell/generic/` | Technology-independent RAM providers for common-cell contracts |

Keep DMA in one native project, with reusable cells in their own library project;
the Rust module tree is not a prescribed RTL hierarchy.
The DMA project now depends on the [local common-cell library](common_cell/README.md),
with an explicit upstream snapshot and no runtime access to the source repository.
Do not create empty pipeline controllers or one project per proposed module.
Firmware stays software under `model/modeling/crates/firmware`; a hardware wrapper
does not own its policy or introduce another software runtime.

Veryl is the design source. Generated SystemVerilog, dependency checkouts,
filelists, simulator caches and waveforms are build artifacts. Use native
Veryl source discovery and dependencies, not generated manifests or copied source
trees. `Veryl.lock` pins the resolved native project dependencies.

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

## Verification and implementation

Use native Veryl tests for local state transitions and interfaces. DMA-level
VCS/UVM/VIP verification belongs in [verification/dma](../verification/dma/README.md).
Synthesis, SDC and CDC/RDC work belongs in [impl/pdma](../impl/pdma/README.md).
Model scenarios remain executable software/hardware semantic references, not
cycle-accurate golden traces.
Initial testbenches drive DMA ports directly and supply memory/fault/timer/IRQ
behavior. They do not require a CPU, synthesized firmware or a recreated managed subsystem.

[Tool integration](../tools/rtl-flow/README.md) independently maintains a fixed
Veryl version and patches, initially taken from a qualified
[pcie_modeling](https://github.com/shady831213/pcie_modeling) snapshot.
Its separate compiler smoke test is not a PDMA design test or EDA qualification.
