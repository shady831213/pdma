# RTL and flow organization review

Date: 2026-11-12. Baseline: `6a2bc763`, the qualified burst-window checkpoint.
This review changes ownership and tooling, not DMA architecture or capabilities.
Synthesizable module/package names, instance hierarchy and Veryl hardware source
bodies remain unchanged. Register, execution and memory tests follow their
corresponding owners.

## Responsibilities

| Owner | Contents |
| --- | --- |
| `rtl/src/dma` | Top-level assembly and generated common layout |
| `rtl/src/dma/{registers,execution,memory}` | Register interface/storage, command/window lifetime, bus endpoints respectively |
| `rtl/common_cell` | Reusable hardware, component-owned constraint queries and passive custody checker definitions |
| `rtl/tech_cell/generic` | Generic RAM providers; no technology deployment |
| `verification/dma/env` | Platform/VIP assembly, software operations, flow lifetime, result checking and passive observations |
| `verification/dma/tests` | Independently selectable DUT/TB scenarios and standalone oracles |
| `verification/dma/tb` | DUT/VIP wiring, explicit binds and generated software-facing packages |
| `impl/dma` | Integration profiles, explicit full-design reviews and design qualification |
| `tools/rtl-flow/implementation` | Generic tool adapters, validation, immutable run inputs and report gates |

Source directories express ownership without introducing extra RTL wrappers.
Imported common-cell hardware/test bodies retain their original provenance;
the project manifest declares the added flow resources separately.

## Findings and fixes

1. **Flat DMA source and test trees.** Register, execution and memory owners now
   have cohesive directories. Existing instance paths remain stable; no compatibility
   source trees, include redirects or symlinks were added.
2. **Implementation flows reached into a particular checkout.** Native Veryl
   metadata resolves package resources into the snapshot. Adapters use that copy,
   with separate provider namespaces and explicit instance bindings. They no longer
   guess common-cell paths, private RAM names or synchronizer structures.
3. **Mixed design and tool ownership.** `impl/dma` selects the DMA profile; shared
   Genus/VC logic lives under tools. The old DC implementation path was removed.
   Earlier DC results remain history, not current qualification.
4. **Mixed verification source roles.** Environment and scenario packages are
   separate. Generated SV lives under `tb/generated`; checker definitions and DUT
   binds have different owners. Software task construction/allocation is separate
   from observed results and passive coverage data. Channels still own rings.
5. **Incomplete process ownership.** Compile, simulation, schema, coverage and
   implementation entries use owned process-group execution/cleanup. Cancellation
   tests include descendants that ignore TERM. Native error diagnostics invalidate
   success markers, including in standalone schema runs.
6. **Obsolete single-outstanding smoke assumption.** The original smoke failed
   with `expected=3 actual=4` writes after reset. Both accepted payload bursts may
   finish under the existing drain contract. The test was retained and corrected:
   generated Copy/Fence records, Fence-based success, retained W-before-AW work,
   busy before B, no successor Fence, no post-reset traffic and a fresh task after
   reset. It does not infer completion from a write counter or require maximum-sized
   bursts. The task-level VIP oracle and protocol assertions remain intact.
7. **Stale documentation and paths.** RAM use, source ownership, generator outputs,
   launch commands and local links now match the implementation. Empty obsolete
   implementation directories were removed; raw evidence stays private.
8. **Application names in reusable resources.** The passive checkers now use
   `common_cell_mailbox_checker` and `common_cell_async_queue_checker`; DMA selects
   their bindings in its testbench. Library documentation describes cell contracts,
   while DMA usage, software reset policy and workspace compiler commands stay with
   the consumer. Generic constraint examples no longer assume DMA instance names.
   Upstream and development provenance remain recorded. No DMA descriptor, ring,
   channel policy or integration waiver is part of the common-cell implementation.

## Package constraints and review validity

The [package contract](../../tools/rtl-flow/ip/README.md) uses existing Veryl
metadata; it adds neither a dependency resolver nor another TOML parser. The entry
defines provider functions only. Loading a dependency does not create clocks,
apply timing exceptions or inherit waivers. Platform clocks/IO/technology remain
integration-owned. Resource files and their identities travel with snapshots.

The complete old review identity was deliberately rejected. New native exports
contain exactly the same 40 CDC and 1363 RDC records, including all compared field
values. Only after that comparison was the existing review rebound to the current
RTL/resource/configuration/tool identities. No finding or rule was removed.
Ordinary exact-path matching and the restricted unconnected-sink group checks
remain in force; wrong paths, missing findings and invalid groups still fail.

## Evidence and limits

Hardware comparison found 39 unchanged generated SV files after source-map
normalization. The remaining file only reordered the independent `RoundRobin<2>`
and `RoundRobin<8>` packages; each named package body is byte-identical. Generated
RTL was not edited. Fresh native tests, system simulations and implementation
checks qualify the current organization rather than relying on that comparison
alone.

The detailed final qualification is recorded in
[verification results](../../verification/dma/results.md). The standalone smoke
has a failing-before/passing-after record. Package tests cover relocation without
the original checkout, version namespace isolation, missing/escaping resources,
duplicates and immutable outputs. Actual mapped integration checks retain the
adjacent-instance, wrong-depth, impossible-budget and false-path negatives.

This remains the one-channel Linear Copy/Fence RTL profile. There is no new CPU,
firmware, peripheral, multichannel or atomic RTL. Mapping and reviewed structural
checks do not establish routed timing, hold/reset recovery/removal, analog MTBF
or workload power signoff.
