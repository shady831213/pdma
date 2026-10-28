# PDMA RTL

This is a directory skeleton, not an implemented or qualified RTL design.
The native Veryl manifest has no design dependencies or executable tests yet.
A successful empty build or zero-test run is not verification evidence.

The [system specification](../model/modeling/docs/spec.md) defines observable
hardware, firmware, Host and platform behavior. The
[RTL architecture draft](../docs/rtl/architecture.md) records confirmed interfaces,
clock/reset requirements and decisions still needed before coding.

## Source ownership

| Path | Contents |
| --- | --- |
| `src/dma/` | DMA channels, shared execution, register behavior and bus endpoints |
| `src/hw_components/` | Managed access windows, context resolution and management/doorbell hardware |
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

Use native Veryl tests for local state transitions and interfaces. System-level
VCS/UVM/VIP verification belongs in [verification/pdma](../verification/pdma/README.md).
Synthesis, SDC and CDC/RDC work belongs in [impl/pdma](../impl/pdma/README.md).
Model scenarios remain executable software/hardware semantic references, not
cycle-accurate golden traces.

[Tool integration](../tools/rtl-flow/README.md) independently maintains a fixed
Veryl version and patches, initially taken from a qualified pcie_modeling snapshot.
Its separate compiler smoke test is not a PDMA design test or EDA qualification.
