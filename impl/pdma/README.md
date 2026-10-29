# PDMA implementation checks

This directory will own Design Compiler synthesis, SDC timing constraints and
VC SpyGlass CDC/RDC analysis for the DMA-only top in the first phase. CPU,
peripheral and subsystem implementation assessments are deferred. Add constraints and flow scripts
with the corresponding design; no timing constraints or waivers are supplied yet.

Use the three domains and reset requirements recorded in the
[architecture draft](../../docs/rtl/architecture.md). The tentative 22nm/500MHz
target is not a library/PVT selection or a timing result. Clock ratios, IO budgets,
storage providers and configuration limits must be explicit in each assessment.

Keep temporary runs in `.run/`. Do not copy another design's CDC/RDC waivers,
license configuration, physical-library paths or qualification claims.
