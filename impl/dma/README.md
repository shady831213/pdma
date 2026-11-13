# DMA implementation checks

These flows assess the DMA-only top. CPU, peripheral and subsystem implementation
remain deferred. The approved target is 12nm with Genus: core and bus clocks at
1GHz (1ns), register clock at 500MHz (2ns), with ULVT forbidden. Equal frequencies
do not establish a synchronous relationship. Library/PVT and electrical budgets
are private inputs; target frequency is not a timing-closure claim.

## Ownership and entries

| Location | Responsibility |
| --- | --- |
| `impl/dma/profiles/copy.tcl` | Top parameters, clocks, port ownership, RAM inventory and coordinated cold-reset assertion |
| `impl/dma/flow/` | DMA entrypoints selecting the profile before delegating to generic adapters |
| `tools/rtl-flow/implementation/profile.tcl` | Tool-independent profile validation and traversal |
| `tools/rtl-flow/implementation/constraints-common.tcl` | Standalone clock SDC and provider/instance traversal |
| `rtl/common_cell/constraints/ip.tcl` | Package-owned scoped IP contracts; integration supplies clocks, hierarchy and budget |
| `tools/rtl-flow/implementation/run.sh` | Fresh run ownership, copied snapshot resources, hashes and failure receipts |
| `tools/rtl-flow/process.sh` | Shared owned-process-group teardown for simulation and implementation |
| `tools/rtl-flow/implementation/genus/` | Genus units, mapping/library policy, constraints, reports and setup gate |
| `tools/rtl-flow/implementation/memory-model.tcl` | Shared capacitance-only RAM boundary library serialization |
| `tools/rtl-flow/implementation/vc/` | VC SpyGlass constraint translation, CDC/RDC analysis and unresolved-finding gate |
| `impl/dma/reviews/copy.tcl`, `copy-8.tcl` | Explicit source-bound dispositions for 2 and 8 channels respectively; never generic block waivers |
| `impl/dma/tests/{genus,vc}/` | Backend-specific qualification fixtures; profile/runner tests remain directly under `tests/` |

The Copy profile accepts `PDMA_CHANNEL_COUNT=1..8` (default 2), including every
non-power-of-two count. Parameters, IRQ widths and RAM geometry are resolved
before tool launch into an immutable `design-profile.tcl`; the source profile
is retained separately. A later environment change cannot reinterpret a run.
The profile describes the actual emitted boundary. A new top can select its own
profile through `PDMA_DESIGN_PROFILE`; it must account for every port. Adapters
fail on missing ports, unexpected bits/directions or incomplete coverage. Genus
flattens SV interface separators; VC retains them. Neither adapter owns a second
DMA interface specification. Optional reset groups describe simultaneous
assertion only; release remains locally synchronous and independent.
Crossing entries select a resolved package provider and cell contract, with a
maximum propagation budget and their two actual domains. Package resources come
from the immutable snapshot, not the current source tree or a guessed cache path.
See [integration and constraint reuse](../../docs/rtl/constraints.md): the standalone wrapper
owns clock creation, while the reusable cell API touches only the named instance.

The profile's optional `memories` inventory names RAM provider instances, port
contracts, dimensions and clocks. Synthesis defaults to `PDMA_MEMORY_MODE=control-budget`;
`generic` retains standard-cell array mapping. The Copy profile excludes one
`(8 * CHANNEL_COUNT)` x 512-bit RAM in budget mode; standard-library FIFO storage remains synthesized.
See [RAM boundary accounting](../../docs/rtl/memory-budget.md) for the generated
library, exact binding checks and the distinction between logical area and
unqualified SRAM cost. Ordinary RTL allows automatic ungrouping and boundary
optimization; RAM leaves and synchronizers retain their separate protection.

Generate an immutable source snapshot, then invoke the desired tool adapter:

```sh
bash tools/rtl-flow/snapshot.sh "$PDMA_RUN_ROOT/snapshot"
PDMA_SYNTH_STAGE=prepare bash impl/dma/flow/genus.sh \
    "$PDMA_RUN_ROOT/snapshot" "$PDMA_RUN_ROOT/prepare"
bash impl/dma/flow/genus.sh "$PDMA_RUN_ROOT/snapshot" "$PDMA_RUN_ROOT/map"
bash impl/dma/flow/crossings.sh "$PDMA_RUN_ROOT/snapshot" "$PDMA_RUN_ROOT/cdc" cdc
bash impl/dma/flow/crossings.sh "$PDMA_RUN_ROOT/snapshot" "$PDMA_RUN_ROOT/rdc" rdc
```

`prepare` validates the library, elaboration and constraints without mapping.
The default `map` stage produces the mapped database/netlist, cell-area, timing,
constraint coverage, reference identities and vectorless power reports. Power
uses explicit tool-default activity parameters (toggle rate 0.02, probability
0.5), not measured workload activity. Setup slack is read from the native timing
object in ps, rather than rounded text; negative slack fails the setup gate.
Hold, routed parasitics, clock-tree implementation, synchronizer MTBF and physical
CDC/reset timing still require separate qualification.
`cdc-timing.tsv` and `cdc-timing-status.json` additionally check that every declared
mapped crossing is timed under its explicit budget, that held-payload paths remain
visible, and that synchronizer stage paths retain normal local timing. The current
0.5ns budgets are not blanket asynchronous false paths. Sync flops are preserved
after mapping; technology-specific synchronizer qualification remains separate.

## Private configuration and reproducibility

See [the flow guide](../../tools/rtl-flow/flows.md) for environment injection.
Genus consumes `PDMA_TECH_CONFIG`, a trusted private Tcl file defining:

- `target_libraries`, `operating_library`, `operating_condition`;
- `time_unit=1e-9`, `capacitance_unit=1e-12`;
- `clock_uncertainty`, `transition`, `io_min`, `io_max`, `output_load` in ns/pF;
- optionally `approved_standard_cell_libraries` (otherwise the operating library).

Clock periods come from the design profile. Library units are checked separately;
Genus database/report time and capacitance use ps/fF. Mapping candidates must be
approved non-ULVT cells, and final instances are audited by actual library/cell
identity plus threshold attributes. Filename filtering alone is insufficient.
The synthetic policy tests exercise forbidden names, hidden cell/library threshold
attributes, unapproved references and empty mappings in the real tool.

Each run has fresh outputs and copied Tcl/profile/launcher inputs. The complete
launcher is parsed before starting a long command. Do not edit an active run's
copies. Input identity is rechecked at exit even after failure; `result.json`
records the flow exit status and `artifacts.sha256` inventories the retained
outputs. A tool timeout or failed report check is not a successful run. Raw logs,
private configurations, library identities and deployment data stay outside Git.
Publish only reviewed summaries without that information.

## CDC/RDC interpretation

VC SpyGlass runs against the actual emitted DUT and common cells, without a
simulation testbench or inherited waivers. RDC follows corruption through
resetless storage to observable consumers and includes asynchronous-clock paths,
primary outputs and disconnected sinks. A dedicated unsafe resetless-chain
fixture verifies this is not blanket exclusion of resetless storage. CDC and RDC
retain raw and unresolved reports; unresolved errors or warnings return failure.
See [reset/CDC obligations](../../docs/rtl/reset-cdc.md#vc-spyglass-cdcrdc-检查门槛).

The Copy profile retains the register mailbox and uses FIFOs for memory traffic.
Gray-pointer decode, independent protocol credits, resetless payloads and exact
unused sinks are reviewed separately in the
[throughput review](../../docs/rtl/throughput-review.md). Select that review deliberately:

```sh
PDMA_STATIC_REVIEW="$PWD/impl/dma/reviews/copy.tcl" \
    bash impl/dma/flow/crossings.sh "$PDMA_RUN_ROOT/snapshot" "$PDMA_RUN_ROOT/cdc-reviewed" cdc
```

The same policy supports `rdc`. Raw findings are reported before exceptions;
`reviewed.rpt`, `unresolved.rpt` and `finding-counts.txt` distinguish the results.
The success marker is `PDMA_STATIC_REVIEWED`, never a claim of raw zero findings.
Exact emitted RTL, source manifest, profile, constraints, analysis configuration
and tool identities must match, including shared constraints and the review engine.
Each ordinary entry matches one distinct finding by native fields. Only a proven
unconnected, no-clock sink may group reset-source paths at that exact endpoint;
the source identity, sink fields and expected distinct-match count are checked.
This rule cannot cover a register or primary output. Unknown findings still fail.
A changed source/configuration needs
a fresh review, not an automatically refreshed digest. No policy is selected by
default. These structural exceptions do not waive physical timing or MTBF.

`cdc-formal` additionally runs the tool's data-hold, data-loss and convergence
properties, with a 60-second limit per property and the ordinary overall timeout.
Inconclusive properties remain findings. `tests/mailbox.sh DUT_SNAPSHOT NEW_PROOF`
uses the same mode on the actual register mailbox and payload types, with free
interface inputs, to separate cell invariants from whole-DMA proof complexity.
It retains the raw structural verdict; proven properties do not silently create
waivers. Formal clock/reset assumptions remain explicit in its selected profile.
An earlier whole-DUT snapshot had one inconclusive datahold property. That result
is not coverage of the new FIFO/window design; no whole-DMA formal completion is
claimed. No inconclusive/formal-error tag is accepted by the structural review.

Tool-sensitivity checks use an independent two-clock fixture: a synchronized
control passes CDC, direct unsynchronized control fails, independent-reset
corruption fails (including through resetless storage), and a bad boundary fails
before analysis. They establish flow sensitivity, not DMA correctness:

```sh
bash impl/dma/tests/run.test.sh "$PDMA_RUN_ROOT/runner-tests"
tclsh impl/dma/tests/profile.test.tcl "$PDMA_RUN_ROOT/profile-tests" "$PDMA_RUN_ROOT/snapshot"
bash impl/dma/tests/genus/genus-policy.sh "$PDMA_RUN_ROOT/policy-tests"
bash impl/dma/tests/genus/memory.sh "$PDMA_RUN_ROOT/snapshot" "$PDMA_RUN_ROOT/memory-tests"
bash impl/dma/tests/genus/memory.sh "$PDMA_RUN_ROOT/snapshot" "$PDMA_RUN_ROOT/memory-minimum-tests" minimum
bash impl/dma/tests/genus/mapped-memory.sh "$PDMA_RUN_ROOT/map" "$PDMA_RUN_ROOT/memory-audit"
bash impl/dma/tests/genus/genus-timing.sh "$PDMA_RUN_ROOT/map" "$PDMA_RUN_ROOT/timing-test"
bash impl/dma/tests/genus/genus-cdc.sh "$PDMA_RUN_ROOT/snapshot" "$PDMA_RUN_ROOT/cdc-integration"
bash impl/dma/tests/vc/crossings.sh "$PDMA_RUN_ROOT/static-tests"
bash impl/dma/tests/vc/review.sh "$PDMA_RUN_ROOT/snapshot" "$PDMA_RUN_ROOT/review-tests"
```

The timing test restores a verified mapped database, confirms the baseline and
deliberately tightens the core period to reject negative slack. It does not change
the design target or producer artifacts. Timed-out or interrupted implementation
and simulation jobs tear down their own process groups; failed runs retain their
input and result records.
Select the matching review with `PDMA_STATIC_REVIEW`; the 8-channel review also
requires `PDMA_CHANNEL_COUNT=8`. A different count or source identity is rejected.
The review tests accept the same explicit policy override.

The review tests use the real unchanged DUT to reject stale identity, a changed
endpoint and an omitted disposition; missing coverage cannot become a pass.

The obsolete DC launcher and its single-period constraints were removed; Git
history retains that evaluation. Do not compare the old and new libraries/tools
as interchangeable area or slack measurements. Public reference flows originate from
[pcie_modeling](https://github.com/shady831213/pcie_modeling); its qualification and
waivers do not transfer to PDMA.
