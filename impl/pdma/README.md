# PDMA implementation checks

These flows assess the DMA-only top. CPU, peripheral and subsystem implementation
remain deferred. The approved target is 12nm with Genus: core and bus clocks at
1GHz (1ns), register clock at 500MHz (2ns), with ULVT forbidden. Equal frequencies
do not establish a synchronous relationship. Library/PVT and electrical budgets
are private inputs; target frequency is not a timing-closure claim.

## Ownership and entries

| Location | Responsibility |
| --- | --- |
| `constraints/dma-copy.tcl` | Fixed top, clock periods, port widths/directions/owners and coordinated cold-reset assertion |
| `flow/profile.tcl` | Tool-independent profile validation and traversal |
| `flow/constraints-common.tcl` | Shared standalone clock SDC and instance/budget traversal |
| `rtl/common_cell/constraints/duplex_mailbox.tcl` | Reusable scoped IP constraints; integration supplies clocks, hierarchy and budget |
| `flow/run.sh` | Fresh run ownership, immutable input copies, hashes and failure receipts |
| `tools/rtl-flow/process.sh` | Shared owned-process-group teardown for simulation and implementation |
| `flow/genus/` | Genus units, mapping/library policy, constraints, reports and setup gate |
| `flow/vc/` | VC SpyGlass constraint translation, CDC/RDC analysis and unresolved-finding gate |
| `constraints/dma-copy-review.tcl` | Explicit, source-bound dispositions for the fixed Copy slice; never a generic block waiver |
| `tests/` | Synthetic tool-sensitivity fixtures and launcher/profile tests |

The profile describes the actual emitted boundary. A new top can select its own
profile through `PDMA_DESIGN_PROFILE`; it must account for every port. Adapters
fail on missing ports, unexpected bits/directions or incomplete coverage. Genus
flattens SV interface separators; VC retains them. Neither adapter owns a second
DMA interface specification. Optional reset groups describe simultaneous
assertion only; release remains locally synchronous and independent.
Mailbox entries declare a maximum propagation budget and their two actual domains.
See [integration and constraint reuse](constraints/README.md): the standalone wrapper
owns clock creation, while the reusable cell API touches only the named instance.

Generate an immutable source snapshot, then invoke the desired tool adapter:

```sh
bash tools/rtl-flow/snapshot.sh "$PDMA_RUN_ROOT/snapshot"
PDMA_SYNTH_STAGE=prepare bash impl/pdma/flow/genus.sh \
    "$PDMA_RUN_ROOT/snapshot" "$PDMA_RUN_ROOT/prepare"
bash impl/pdma/flow/genus.sh "$PDMA_RUN_ROOT/snapshot" "$PDMA_RUN_ROOT/map"
bash impl/pdma/flow/crossings.sh "$PDMA_RUN_ROOT/snapshot" "$PDMA_RUN_ROOT/cdc" cdc
bash impl/pdma/flow/crossings.sh "$PDMA_RUN_ROOT/snapshot" "$PDMA_RUN_ROOT/rdc" rdc
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
PDMA_STATIC_REVIEW="$PWD/impl/pdma/constraints/dma-copy-review.tcl" \
    bash impl/pdma/flow/crossings.sh "$PDMA_RUN_ROOT/snapshot" "$PDMA_RUN_ROOT/cdc-reviewed" cdc
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
bash impl/pdma/tests/run.test.sh "$PDMA_RUN_ROOT/runner-tests"
tclsh impl/pdma/tests/profile.test.tcl "$PDMA_RUN_ROOT/profile-tests"
bash impl/pdma/tests/genus-policy.sh "$PDMA_RUN_ROOT/policy-tests"
bash impl/pdma/tests/genus-timing.sh "$PDMA_RUN_ROOT/map" "$PDMA_RUN_ROOT/timing-test"
bash impl/pdma/tests/genus-cdc.sh "$PDMA_RUN_ROOT/snapshot" "$PDMA_RUN_ROOT/cdc-integration"
bash impl/pdma/tests/crossings.sh "$PDMA_RUN_ROOT/static-tests"
bash impl/pdma/tests/review.sh "$PDMA_RUN_ROOT/snapshot" "$PDMA_RUN_ROOT/review-tests"
```

The timing test restores a verified mapped database, confirms the baseline and
deliberately tightens the core period to reject negative slack. It does not change
the design target or producer artifacts. Timed-out or interrupted implementation
and simulation jobs tear down their own process groups; failed runs retain their
input and result records.
The review tests use the real unchanged DUT to reject stale identity, a changed
endpoint and an omitted disposition; missing coverage cannot become a pass.

The older `flow/synthesize.sh` and `constraints/timing.tcl` retain the historical
DC evaluation with a single `clock_period` input. They do not implement this new
target. Do not compare the old and new libraries/tools as interchangeable area or
slack measurements. Public reference flows originate from
[pcie_modeling](https://github.com/shady831213/pcie_modeling); its qualification and
waivers do not transfer to PDMA.
