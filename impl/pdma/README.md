# PDMA implementation checks

This directory documents implementation obligations for the DMA-only top in the
first phase. CPU, peripheral and subsystem assessments are deferred. Commercial
tool flows, deployment details, library inputs and reports are maintained outside
the repository. No timing constraints or waivers are supplied here yet.

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
