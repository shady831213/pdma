# Reusing PDMA timing constraints

There are two ownership levels. The reusable cell constraints belong to the IP;
clock creation, global relationships, external delays, reset sequencing and
technology/electrical budgets belong to the integrating platform.

| File | Role |
| --- | --- |
| `rtl/common_cell/constraints/duplex_mailbox.tcl` | Reusable instance-scoped handshake timing API; no clock creation or global exceptions |
| `rtl/common_cell/constraints/async_queue.tcl` | Reusable FIFO data/Gray-pointer budgets and synchronizer-chain metadata |
| `rtl/common_cell/constraints/ip.tcl` | Veryl metadata entry exporting the package's contracts; no automatic constraint or waiver application |
| `impl/dma/profiles/copy.tcl` | Copy integration: port ownership, clocks, reset contract, provider/instance bindings, depths and maximum delays |
| `tools/rtl-flow/implementation/constraints-common.tcl` | Standalone clock SDC and explicit provider invocation |
| `tools/rtl-flow/implementation/genus/{timing,cdc}.tcl` | Electrical constraints and mapped timing/structure audit |
| `tools/rtl-flow/implementation/vc/constraints.tcl` | RTL clock/reset/interface intent and structural-analysis setup |
| `impl/dma/reviews/copy.tcl` | Reviewed findings for one exact design/configuration; not a reusable whole-IP waiver |

Paths in this table are repository-relative. Native Veryl metadata resolves
package paths and revisions; snapshots carry their declared constraint resources.
The generic adapters load that snapshot copy. Provider namespaces isolate multiple
versions, and the profile applies contracts once per actual instance. See the
[package API](../../tools/rtl-flow/ip/README.md). The historical DC entry is in Git history.

## Integrating an instance

The application API accepts a literal hierarchy path and the integration's actual
clock collections. The budget uses the active SDC time unit. Standalone flows set
that unit to ns; an integrator must establish its own consistent units and budget.

```tcl
# COMMON_CELL_ROOT comes from resolved metadata (or a bundled resource root).
source [file join $COMMON_CELL_ROOT constraints ip.tcl]

common_cell_cdc::duplex_mailbox soc/dma0/csr/crossing \
    [get_clocks cfg_clock] [get_clocks dma_clock] $register_crossing_budget
common_cell_cdc::async_queue soc/dma0/request_crossing \
    [get_clocks dma_clock] [get_clocks memory_clock] $memory_crossing_budget
common_cell_cdc::async_queue soc/dma0/response_crossing \
    [get_clocks memory_clock] [get_clocks dma_clock] $memory_crossing_budget
```

The script defines procedures only; sourcing it does not modify the design.
Each call selects registers inside the specified instance, in the supplied clock
domains, and constrains only its actual cross-domain launch/capture pairs. Literal
hierarchy matching prevents a scope such as `ip0` from selecting `ip01`. Register
collections avoid assuming mapped flop names, payload bit numbering or cell pins.
Empty bindings and invalid clock/budget arguments fail.

For the present `DuplexMailbox`, each direction uses a fixed two-stage control
synchronizer. The `synchronizers` query returns those registers and rejects a
different shape. The integrating synthesis/implementation flow must retain their
sequential structure and timing; the Genus adapter disables retiming, preserves
hierarchy, then applies `set_dont_touch` to these mapped registers before `syn_opt`.
This is independent of technology-specific synchronizer-cell selection and MTBF
qualification, which are not supplied by an SDC maximum-delay constraint.

```tcl
# Genus adapter, after technology mapping and before subsequent optimization:
set sync_cells [common_cell_cdc::synchronizers soc/dma0/csr/crossing \
    [get_clocks cfg_clock] [get_clocks dma_clock]]
set_dont_touch $sync_cells
set chains [common_cell_cdc::async_queue_chains soc/dma0/request_crossing \
    [get_clocks dma_clock] [get_clocks memory_clock] 8]
set_dont_touch [common_cell_cdc::synchronizer_cells $chains]
# Repeat for the response queue with its reversed clocks and actual depth.
```

Integration must preserve these logical instance boundaries or provide an audited
binding for its transformed hierarchy. Do not silently broaden a failed selector
to the entire chip. After mapping, re-resolve with `duplex_paths` to inspect the
actual path sets (`async_queue_paths` for a FIFO); do not retain stale tool
collections across netlist transforms. Cell-owned helpers define their private
synchronizer hierarchy; application adapters do not duplicate those paths.

## Timing contract

The current profile specifies 0.5ns for mailbox and FIFO crossings in both directions. This is
half the fastest 1ns local period. Source payload and request state are registered
together; the request traverses two destination synchronizer stages before local
qualified capture. The budget conservatively leaves time for payload arrival
before that capture. It is a design requirement, not a measured routed delay and
not a universal constant for other clock ratios or handshake implementations.

`set_max_delay -ignore_clock_latency` bounds the scoped register-to-register paths,
including payload and request/ACK controls. Cross-domain hold checks are excluded
only for those pairs; unrelated clocks have no fixed phase relationship. Local
synchronizer stage paths remain under their normal clock's setup/hold constraints.
The standalone validator rejects a mailbox budget exceeding either local period.

Global asynchronous groups or other false-path exceptions must not hide these
maximum-delay checks. The standalone clock SDC uses `-asynchronous -allow_paths`,
supported by both selected tools. The IP script does not remove or override an
integrator's global exceptions. The integrator must resolve conflicting exceptions
and prove that real paths remain analyzed. No-path reports are failures, not passes.

The Genus audit checks each mapped crossing, explicitly checks the held-payload
paths, verifies the budget shown in the native report and checks native slack.
It separately checks all local synchronizer stage paths remain timed without the
cross-domain exception. Pre-layout estimates still omit routed parasitics, clock
tree effects, reset recovery/removal signoff and analog MTBF qualification.

For `AsyncQueue`, registered Gray pointers change by one count per accepted push
or pop. The absolute crossing budget is below the faster clock period, bounding
both propagation and Gray-bus skew; pointer synchronizers are preserved and their
local stage paths remain timed. The data bank stays owned until the synchronized
read pointer releases its row. The audit separately checks actual data-bank paths,
not only pointer paths. Queue depth must match the instantiated power-of-two depth;
the synchronizer-width check rejects an incorrect profile declaration.

A CDC structural waiver is not an STA false-path. Reconvergent control logic
after synchronization remains ordinary local-clock logic and keeps its single-cycle
setup/hold requirements. The current cross-domain exceptions retain explicit
maximum-delay checks and cut only phase-based cross-domain hold checks; payload
hold is guaranteed by the handshake's retained custody until acknowledgement.

The CDC literature's multi-cycle/bundled-data transfer is not automatically an
SDC `set_multicycle_path`. That command changes the launch/capture edge relationship
used by STA. It needs a justified capture-enable/edge contract and corresponding
hold treatment; two arbitrary asynchronous clocks do not acquire such a relation
merely because a synchronizer takes multiple cycles. Do not relax the local
reconvergence logic or replace the current absolute crossing budgets with a guessed
multicycle value. A future true local N-cycle path must be reviewed on its own
functional sampling contract. The vendor
[UG903 constraint guide](https://docs.amd.com/api/khub/documents/eG7Sc4SvBBo6KERDMQTuMA/content)
discusses multicycle edge relationships and asynchronous maximum-delay constraints
separately in Chapter 5; tool-specific options and exception priorities still
require native validation.

## What Genus and VC SpyGlass share

Both consume the same design profile and the same generated `clocks.sdc` through
`read_sdc`. Clock names, periods and asynchronous relationships have one source.
The common-cell constraint API can be reused by synthesis and STA flows without
changing the integration's clock/IO/reset ownership.

The RTL VC database does not expose the mapped register pin timing graph used by
STA. The VC adapter records the shared instance/domain/budget intent explicitly,
then checks RTL structure, reset relationships and interface ownership. It does
not claim to verify physical propagation delay by merely accepting SDC syntax.
Those paths are bound and checked in Genus/STA. Reset and CDC-specific attributes
remain native VC Tcl; external numerical IO/electrical budgets remain platform
inputs to synthesis. There is no silent empty physical-path application in VC.

The standalone wrapper creates clocks and external delays; do not source/run that
wrapper inside an SoC that already owns them. Reuse the cell API above instead.

## Qualification

```sh
bash impl/dma/tests/genus/genus-cdc.sh "$PDMA_RUN_ROOT/snapshot" "$PDMA_RUN_ROOT/cdc-integration"
```

The real mapped test has a mailbox and FIFO pair in two nested IP instances on
the same clocks, with separate 0.5ns and 0.7ns budgets. It checks isolation, timed
local synchronizer stages, missing instances, wrong FIFO depth, impossible 0.001ns
budgets for both transport types and an overriding false-path.
The production DMA mapping has its own audit; the small test does not substitute
for that result. Commercial raw reports and library details stay outside Git.
