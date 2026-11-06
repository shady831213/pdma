# PDMA implementation checks

This directory documents implementation obligations for the DMA-only top in the
first phase. CPU, peripheral and subsystem assessments are deferred. Reproducible
flow code lives in `flow/`, with generic three-clock timing constraints under
`constraints/`. Deployment details, library inputs and raw reports stay outside
version control. See [the flow guide](../../tools/rtl-flow/flows.md).

Use the three domains and reset requirements recorded in the
[architecture draft](../../docs/rtl/architecture.md). The tentative 22nm/500MHz
target is not a library/PVT selection or a timing result. Clock ratios, IO budgets,
storage providers and configuration limits must be explicit in each assessment.

Continue with the existing evaluation environment while the DMA microarchitecture
converges; a future environment migration is not a prerequisite. Re-establish the
physical baseline when migrating instead of mixing measurements across flows.
Keep commercial implementation runs outside the repository. Do not copy another
design's CDC/RDC waivers, license configuration, physical-library paths or
qualification claims.

`flow/synthesize.sh SNAPSHOT NEW_RUN` consumes a source snapshot from
`tools/rtl-flow/snapshot.sh` and a private `PDMA_TECH_CONFIG` Tcl file. The Tcl
file provides `target_libraries`, `operating_library`, `operating_condition`,
`time_unit`, `capacitance_unit`, `clock_period`, `clock_uncertainty`, `transition`,
`io_min`, `io_max` and `output_load`. The public flow checks library units, mapping,
design consistency and input hashes, then writes timing/area/power reports and
the applied SDC. The current DC flow estimates synchronous mapped timing with
asynchronous clock groups and reset false paths; it is not CDC/RDC signoff.
Power uses declared vectorless activity, not a measured workload.
