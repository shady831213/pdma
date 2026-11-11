# PDMA implementation checks

This directory documents implementation obligations for the DMA-only top in the
first phase. CPU, peripheral and subsystem assessments are deferred. Reproducible
flow code lives in `flow/`, with generic three-clock timing constraints under
`constraints/`. Deployment details, library inputs and raw reports stay outside
version control. See [the flow guide](../../tools/rtl-flow/flows.md).

Use the three domains and reset requirements recorded in the
[architecture draft](../../docs/rtl/architecture.md). The approved target is
12nm with Genus synthesis: core and bus clocks each at 1GHz (1ns), register clock
at 500MHz (2ns), with ULVT forbidden. Equal core/bus frequencies do not establish
a synchronous relationship. This target is not a library/PVT selection or a timing
result. IO budgets, storage providers and configuration limits must be explicit.

Re-establish the physical baseline with the new flow instead of mixing it with
the historical 22nm/500MHz evaluation. Select approved non-ULVT libraries, disable
forbidden mapping candidates and audit final mapped library/cell identities and
threshold-voltage attributes, including inserted cells. Filename filtering alone
is insufficient. Keep commercial implementation runs outside version control.
Do not copy another design's CDC/RDC waivers, deployment configuration or
qualification claims. Public flow references come from
[pcie_modeling](https://github.com/shady831213/pcie_modeling); its frozen-design
evidence does not qualify PDMA.

`flow/synthesize.sh SNAPSHOT NEW_RUN` consumes a source snapshot from
`tools/rtl-flow/snapshot.sh` and a private `PDMA_TECH_CONFIG` Tcl file. The Tcl
file provides `target_libraries`, `operating_library`, `operating_condition`,
`time_unit`, `capacitance_unit`, `clock_period`, `clock_uncertainty`, `transition`,
`io_min`, `io_max` and `output_load`. The public flow checks library units, mapping,
design consistency and input hashes, then writes timing/area/power reports and
the applied SDC. The current DC flow estimates synchronous mapped timing with
asynchronous clock groups and reset false paths; it is not CDC/RDC signoff.
Power uses declared vectorless activity, not a measured workload.

The checked-in launcher above still implements the historical DC flow and one
common clock-period input. Selecting a different executable does not convert its
Tcl to Genus or implement the new per-domain constraints. The Genus port, explicit
three-period constraints and non-ULVT guard qualification remain to be completed
before claiming the new baseline. Preserve library-unit checks when converting
the 1ns/1ns/2ns targets to the tool's units.

CDC and RDC must both run in VC SpyGlass against the actual emitted DMA top and
its common cells. Explicit clock/reset and boundary ownership constraints, negative
fixtures, retained raw findings and reviewed narrow exceptions are required; see
[the reset/CDC obligations](../../docs/rtl/reset-cdc.md#vc-spyglass-cdcrdc-检查门槛).
The PDMA VC SpyGlass flow and its qualification are pending. Neither randomized
three-clock simulation nor asynchronous STA clock groups establish CDC/RDC closure.
