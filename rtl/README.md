# PDMA RTL

This is a directory skeleton, not an implemented or qualified RTL design.
The native Veryl manifest has no design dependencies or executable tests yet.
A successful empty build or zero-test run is not verification evidence.

The first RTL phase is **DMA only**, confirmed 2026-10-29. CPU implementation,
managed peripheral hardware, firmware execution and full subsystem integration
are deferred until DMA RTL converges. Keep external control/memory/notification
interfaces suitable for that integration; do not build those external owners now.

The [system specification](../model/modeling/docs/spec.md) defines observable
hardware, firmware, Host and platform behavior. The
[RTL architecture draft](../docs/rtl/architecture.md) records confirmed interfaces,
clock/reset requirements and decisions still needed before coding.
The [register-interface proposal](../docs/rtl/register-interface.md) develops the
32/64-bit access, submission and CDC rules; it remains a proposal, not implemented RTL.
The related [memory interface](../docs/rtl/memory-interface.md),
[datapath](../docs/rtl/datapath.md) and [reset/CDC](../docs/rtl/reset-cdc.md) drafts
define transaction ownership and identify correctness constraints on pipelining.

## Source ownership

| Path | Contents |
| --- | --- |
| `src/dma/` | DMA channels, shared execution, register behavior and bus endpoints |
| `src/hw_components/` | Deferred placeholder for managed peripheral hardware; outside the first RTL phase |
| `src/tests/` | Native Veryl block tests and their local support |

Start with one native project. Add subdirectories and shared packages when real
modules require them; the Rust module tree is not a prescribed RTL hierarchy.
Do not create empty pipeline controllers or one project per proposed module.
Firmware stays software under `model/modeling/crates/firmware`; a hardware wrapper
does not own its policy or introduce another software runtime.

Veryl is the design source. Generated SystemVerilog, dependency checkouts,
filelists, simulator caches and waveforms are build artifacts. Use native
Veryl source discovery and dependencies, not generated manifests or copied source
trees. A lockfile will be produced when dependencies are actually resolved.

## Verification and implementation

Use native Veryl tests for local state transitions and interfaces. DMA-level
VCS/UVM/VIP verification belongs in [verification/pdma](../verification/pdma/README.md).
Synthesis, SDC and CDC/RDC work belongs in [impl/pdma](../impl/pdma/README.md).
Model scenarios remain executable software/hardware semantic references, not
cycle-accurate golden traces.
Initial testbenches drive DMA ports directly and supply memory/fault/timer/IRQ
behavior. They do not require a CPU, synthesized firmware or a recreated managed subsystem.

[Tool integration](../tools/rtl-flow/README.md) independently maintains a fixed
Veryl version and patches, initially taken from a qualified pcie_modeling snapshot.
Its separate compiler smoke test is not a PDMA design test or EDA qualification.
