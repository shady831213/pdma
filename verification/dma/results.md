# Scenario-level verification checkpoints

## AXI endpoint extraction — local review candidate

The local `bus_cell` candidate was exercised with the DMA core and with an
independent consumer. The latter carried a high 48-bit register address,
48-bit memory address, six USER bits, an 11-bit tag and 128-bit read data
through emitted SystemVerilog. Both native simulators passed the four endpoint
tests and 24 DMA tests. An isolated checker test held a transaction stalled and
detected a deliberate change to its payload.

| DUT regression | Positive runs | Expected checker rejections |
| --- | ---: | ---: |
| 14 configurations from the preceding checkpoint | 448 | 112 |
| Default two-channel profile with the AXI4 memory port | 32 | 8 |
| Eight-channel synchronizer-delay stress | 16 | 8 |

The shared endpoint preserved the zero-delay 16 KiB Copy measurement on both
default channel counts and on the AXI4 variant: 2,048 cycles for 2,048 data
beats in each direction, and 2,161 bus cycles from descriptor to Fence. The
eight-channel build reproduced all 14 previously measured latency distributions
at the same seed. All 256 eight-channel positive runs contributed to the merged
coverage database, which hit all 18 PDMA scenario bins.

| Channels | CDC raw / reviewed / unresolved | RDC raw / reviewed / unresolved |
| ---: | ---: | ---: |
| 2 | 66 / 66 / 0 | 1,517 / 1,517 / 0 |
| 8 | 210 / 210 / 0 | 2,369 / 2,369 / 0 |

Flattening the opaque tag and address attributes changes how the tool groups
RDC findings. Expanding the reports to bit endpoints reproduced the previous
64,486 and 105,010 source/sink pairs for two and eight channels respectively.
Separate connectivity queries confirmed zero fanout at the affected unused
sinks. The standalone register mailbox CDC run reported zero findings. These
reviews are bound to the local dependency graph. A released dependency and
lockfile require a fresh identity check.

At RTL checkpoint `beb343fe` with the local `bus_cell` sources from
`debfcf6`, all four control-budget synthesis profiles met the 1 GHz core
and bus and 500 MHz register targets. The table compares final mapped
results with the corresponding pre-extraction profile and technology inputs.

| Profile | Logic area change | Vectorless power change | Worst setup slack |
| --- | ---: | ---: | ---: |
| 1 channel, 32-bit memory | +0.061% | -0.369% | 0 ps |
| 2 channels, 64-bit memory | -0.340% | -0.651% | 0 ps |
| 3 channels, 128-bit memory | +0.009% | +0.072% | 0 ps |
| 8 channels, 64-bit memory | +0.006% | +0.179% | 0 ps |

Each profile retained the same number of sequential cells as its baseline.
The restored mapped databases passed the memory qualification and tightened-
clock rejection checks. Power estimates use default activity. The RAM leaf
models boundary loading only. Physical SRAM and routed timing remain open.

## 2026-11-19 core timing follow-up

Each write-one action now takes its pulse directly from the decoded channel
register. The Copy window selects a forwarding candidate from the returned row
and gates its transfer with the existing stop/valid handshake. Queue depths,
clock periods and pipeline cuts are unchanged.

Both native backends passed all 27 tests. A differential simulation compared
the previous and revised channel register modules on identical inputs across
114,688 combinations of address, access width, write strobe and lifecycle
controls. All response, state and action outputs matched. The admin reset
still fired on valid writes while work-register writes were blocked.

| Channels | CDC raw / reviewed / unresolved | RDC raw / reviewed / unresolved |
| --- | --- | --- |
| 2 | 66 / 66 / 0 | 1805 / 1805 / 0 |
| 8 | 210 / 210 / 0 | 2819 / 2819 / 0 |

The new RDC reports remove 358 and 1,352 source-to-sink paths respectively.
Comparison of the native endpoint and reset fields found no added paths.
Fresh connectivity queries found zero endpoint fanout on each affected sink
and confirmed that the selected channel and owner registers still have consumers.
All seven source-bound review tests passed, including the stale-identity and
missing-sink rejections.

At 128-bit memory width, the original testbench could not generate a Copy
starting on lane 15. The random task and its alignment coverage now use the
configured beat width. The last-lane scenario checks the allocated addresses
and the copied bytes with the source and destination taking turns on that lane.
At 1024 bits it reaches lane 127. Error tests span the injected address after
the page boundary, and fault-replay tests change only the bytes retained by
the failed write. Delaying the Fence write address by the Fence64 beat count plus
two cycles exercises W-before-AW even with a payload capacity of 256 beats.

A separate page-aligned 2KiB Copy on the 64-bit port produced one 256-beat
read and one 256-beat write. Both directions sustained eight bytes per cycle
within that burst. The scoreboard observed the Fence and verified destination
bytes and guard bytes. This single-burst observation does not measure the gap
between consecutive 256-beat bursts.

The default eight-channel build passed 32 scenarios at eight seeds each. All
256 positive runs contributed to the merged coverage database. The PDMA scenario
group hit 18/18 bins, and eight independent checker mutations were rejected.
The separate synchronizer-delay build passed 16 scenarios and rejected eight
mutations. At the matched default two- and eight-channel settings, the 16KiB
zero-delay Copy retained 2,048 beats in 2,048 cycles per direction and 2,161
cycles from descriptor to Fence. The eight-channel timing probe measured 4.18ns
from core issue to endpoint and 4.82ns from return FIFO to core.

Across the default profiles for channel counts 1 through 8 and the narrow
one-channel and wide three-channel profiles, the final testbench passed 408
positive runs and rejected 80 checker mutations. This total includes the 256
eight-channel runs above. The four limit profiles below bring the total to
448 positive runs and 112 expected rejections across 14 configurations.

Four additional profiles each passed ten Copy, lane, register, error and
fault-recovery scenarios and rejected all eight checker mutations:

| Channels | Memory width | Maximum beats per burst | Slots per channel |
| ---: | ---: | ---: | ---: |
| 3 | 256 bits | 3 | 3 |
| 2 | 512 bits | 8 | 5 |
| 1 | 1024 bits | 1 | 1 |
| 1 | 64 bits | 256 | 1 |

All four used 32-bit registers and eight read and write credits. These are
functional results; they establish no PPA or sustained-throughput guarantee
for the four profiles.

The following control-budget synthesis runs use the RTL in `9500c28c`. The
core and bus targets are 1GHz and the register target is 500MHz. Each run
passed the mapped-cell policy and the instance-scoped crossing checks.

| Profile | Worst setup slack | Mapped instances | Sequential instances | Logic cell area | Min. crossing slack | Min. RAM boundary slack |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Narrow, 1 channel | 0ps | 20,938 | 4,603 | 6,850.068 | 262.4ps | 277.6ps |
| Default, 2 channels | 0ps | 70,627 | 23,571 | 29,500.674 | 220.3ps | 65.9ps |
| 128-bit memory, 3 channels | 0ps | 69,820 | 16,688 | 24,188.609 | 249.3ps | 193.4ps |
| Default, 8 channels | 0ps | 157,302 | 40,143 | 56,966.713 | 207.1ps | 101.2ps |

The three-channel core now meets setup at the same constraints that gave
−43.2ps at `4de9cb7f`. Only the channel-register and Copy-window modules
changed in the emitted RTL for that profile. Relative to the preceding
matched runs, logic area decreased 20.12% on the three-channel profile, 3.03%
on the default two-channel profile and 0.41% on the default eight-channel
profile. It increased 1.96% on the narrow one-channel profile. All four
results are pre-layout logic estimates. The RAM model excludes intrinsic
SRAM timing, area and power.

Reopening each of the four saved databases reproduced the reported worst
slack and passed the RAM-load, hierarchy, mapped-cell and crossing checks.
Tightening the core clock in each restored database produced −100ps worst
slack and the timing gate rejected all four cases.

## 2026-11-18 configurable ownership boundaries

This candidate separates `DmaControl`, `CopyEngine` and `DmaMemoryBridge` without
adding a pipeline cut. The [configuration contract](../../docs/rtl/configuration.md)
defines independent inputs and derived geometry. RTL, VIP/checkers and
implementation use the same immutable snapshot profile.

The frozen eight-channel RTL passes all **31 scenarios at eight seeds: 248 positive
runs**, plus all eight checker mutations with their expected diagnostics. Separate
synchronizer-latency stress passes 16 positive runs and rejects the same eight
mutation classes. The native suite passes 27 tests, including directional credits,
narrow/full bus lanes, one/non-binary slot counts and Fence64 on a 32-bit port.

Fresh source-bound CDC/RDC checks pass at both required channel counts:

| Channels | CDC raw / reviewed / unresolved | RDC raw / reviewed / unresolved |
| --- | --- | --- |
| 2 | 66 / 66 / 0 | 2163 / 2163 / 0 |
| 8 | 210 / 210 / 0 | 4171 / 4171 / 0 |

All seven native review-gate tests pass, including rejection of stale input
identity, incorrect/missing endpoints and invalid grouping. The new source
inventory is independent of checkout/run location while retaining native compiler
options, source contents, lockfiles and package-owned resources. Snapshot byte
checks remain separate; relocation is not permission to reuse different RTL.

The 248-run coverage merge preserves every input and adds no exclusions. The
declared PDMA scenario group covers 18/18 bins. Selected module line/condition/
branch coverage is:

| Module | Line | Condition | Branch |
| --- | ---: | ---: | ---: |
| CopyEngine | 89.74% | 74.45% | 81.94% |
| CopyWindow | 97.73% | 83.06% | 97.44% |
| CopyReadEndpoint | 100% | 81.82% | 100% |
| CopyWriteEndpoint | 100% | 81.97% | 100% |

This finite scenario group is not exhaustive architecture coverage. Raw reports
retain VIP coverage, unexercised protocol-error paths and reserved bits; neither
the scenario score nor module line coverage claims full parameter-space coverage.

At the unchanged default 64-bit memory profile, aligned 16KiB Copy still transfers
2048 read beats and 2048 write beats in 2048 cycles per direction, with zero payload
bubbles. Descriptor-to-Fence latency remains 2161 bus cycles. The internal observer
also retains the previous 4.18ns issue-to-endpoint and 4.82ns receipt-to-core values;
the arbitration-to-issue register path remains two core cycles.

The final functional matrix passes **391 positive runs**, with eight checker
mutations rejected in each of its ten configurations:

| Profile | Positive runs | Scope |
| --- | ---: | --- |
| Default, 1 channel | 6 | Copy, random traffic, reset, ring wrap and registers |
| Default, 2 channels | 31 | Full scenario suite |
| Default, 3..7 channels, each | 12 | Count boundaries and independent peer lifecycle |
| Default, 8 channels | 248 | Full suite, eight seeds |
| 3 channels, 64-bit registers / 128-bit memory | 26 | Three slots, read/write credits 2/3, FIFO depths 4/4 |
| 1 channel, 32-bit memory | 20 | Three beats, one slot, read/write credits 1/2, FIFO depths 4/4 |

The first narrow-profile implementation preparation rejected FIFO depth two:
the pinned cell's constraint contract requires at least four. The resolver now
rejects that input before tool launch. The supported narrow profile was rebuilt
and all 20 scenarios plus eight negatives rerun; the earlier depth-two simulation
does not qualify a supported integration. Six resolver tests pass, and the native
implementation-profile checks reject 27 invalid configurations and six invalid
channel counts while accepting every count in 1..8. Widths 256/512/1024 and the
256-beat boundary have elaboration evidence only, not functional or PPA closure.

Genus control-budget synthesis of checkpoint `4de9cb7f` met setup for three configurations. Core/bus
remain at 1GHz, registers at 500MHz, with no ULVT and no relaxed crossing budgets:

| Profile | Worst setup slack | Mapped instances | Sequential instances | Logic cell area (library units) | Minimum crossing slack | Minimum RAM boundary slack |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Default, 2 channels | 0ps | 80,292 | 23,571 | 30,423.749 | 224.9ps | 75.6ps |
| Default, 8 channels | 0ps | 158,771 | 39,913 | 57,202.532 | 199.7ps | 102.2ps |
| Narrow, 1 channel | 0ps | 20,360 | 4,604 | 6,718.685 | 262.3ps | 223.2ps |

Each passes eight crossing groups, four held-payload checks and eight local
synchronizer chains; minimum local-chain slack is 820.6ps. At matched default
two-channel settings and technology inputs, logic area increases **2.01%** from
29,824.451. Throughput and command latency remain unchanged; this is not a claim
of cost-free parameterization. The narrow profile has different resources and is
not a like-for-like area improvement over that baseline.

At eight channels, logic area decreased **14.26%** from 66,717.573 under the
matched constraints. Both default-profile restored databases passed the RAM, load,
boundary and hierarchy audits and rejected the tightened core clock.

The three-channel profile with 64-bit registers and a 128-bit memory port missed
core setup by **43.2ps**. Its bus and register clock groups met setup, and all
mapped crossing checks passed. In 18 of the 20 worst paths the register request
decode fed the core-side memory-request FIFO. The other two paths ran from
event-ring capacity to channel fatal state.
Physical SRAM area, power and intrinsic timing remain outside
the control-budget model. Zero worst slack provides no extra timing margin.

## 2026-11-16 implemented microarchitecture and timing observations

The [microarchitecture](../../docs/rtl/microarchitecture.md) maps the current RTL's
owners, arbitration, FSMs, pipeline cuts and lifecycle timing. The separate
[performance report](../../docs/rtl/performance.md) defines and records payload,
command/batch and internal-boundary measurements.

Fresh 2/8-channel builds passed 16 runs in total: five timing scenarios and one
observer-disabled case per configuration, two aggregate multichannel runs, and
write-fault/multichannel-reset runs checking observer custody. The ten original
throughput observations exactly match the observer-free baseline. Production RTL,
constraints and instance hierarchy did not change. No synthesis or CDC/RDC rerun
is claimed for this documentation/observation change. Raw logs and private tool
configuration remain outside tracked content.


## 2026-11-16 multichannel closure

The normal two-channel mapped diagnostic reports roughly −344ps worst setup
slack before optimization. A separate register-to-register query also reports
−344ps from a command-ring consumer to request-FIFO address storage; this is not
solely a reset-input path. Full baseline optimization completed with −186.3ps
worst setup slack, failing the 1GHz target; library and crossing-budget checks
passed. The timing-cut version, before the later decode correction, subsequently
reached 0ps with all implementation gates passing. That is diagnostic evidence,
not qualification of the final source.

The execution pipeline registers the arbitration grant and retains each channel's current
control-request address before issue. Event publication has an explicit address
preparation phase. No clock, crossing budget, library policy or external protocol
was relaxed. Both native compiler backends pass 21/21 tests; the local 64-byte
Copy fixture retains its 10/12 bus-cycle results without/with backpressure. The
Fence resume fixture now waits within its existing request timeout instead of
assuming immediate arbitration, retaining every target/value/width/context check.

Decode review also reproduced malformed Fence acceptance: an invalid width
latched CommandRead fatal but still advanced the consumer. Acceptance and fatal
handling now share one validity predicate. Native tests also reject nonzero
Copy/Fence direct gaps and inactive union bytes before consumption, execution or
event publication. The unsupported-command scenario now constructs a valid Trap
record instead of changing the opcode on a Copy body.

Fresh two-channel regression passes 62 positive runs and rejects all eight checker
mutations. Long-copy read/write throughput remains 8 B/bus cycle with zero payload
bubbles. At matched settings, short/long descriptor-to-Fence latency increases
from 120/2160 to 121/2161 bus cycles; the extra admission register is visible at
command boundaries, not repeated for every payload burst.

Fresh source-bound gates accept CDC 66/66 and RDC 2163/2163, with zero unresolved
findings. The changed two-channel tag slices are checked against exact disabled
FIFO data-match pins: they have no endpoints and their flat traversal contains
only the queried input pin. The named unused arbitration intermediate has zero
fanout; live chosen/owner nets remain connected. The old source binding is rejected.
The final eight-channel regression passes all 248 positive runs and rejects eight
checker mutations. Its fresh CDC/RDC gates accept 210/210 and 4171/4171 findings,
with zero unresolved items; all 650 cited disabled FIFO inputs and RAM collision
nodes have no downstream endpoints. These remain path-specific reviews.
The full coverage merge preserves all 248 source runs: PDMA scenario bins 18/18,
top-level line 2596/2809 (92.42%), condition 1975/2983 (66.21%), branch 1240/1552
(79.90%) and toggle 87707/907796 (9.66%). No metric exclusions were added.

The retained matrix is complete: 31 positive runs at one channel, 62 at two,
18 each at three through seven, and 248 at eight: **431 positive runs**. Each
configuration rejects the eight distinct checker mutations; supplemental batches
repeat some negative controls and do not add new mutation classes. Eight-channel
synchronizer-latency stress adds 18 positive runs and eight rejected mutations.
All seven native review-gate checks pass on the final eight-channel source.

Fresh final-source control-budget mapping and restored-database audits pass at
both required implementation configurations:

| Configuration | Worst setup slack | Mapped instances | Sequential instances | Logic cell area (library units) | Minimum crossing slack | Minimum RAM boundary slack |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 2 channels | 0ps | 73,196 | 23,575 | 29,824.451 | 223.4ps | 49.4ps |
| 8 channels | 0ps | 175,196 | 40,147 | 66,717.573 | 191.8ps | 44.7ps |

Both use core/bus at 1GHz and registers at 500MHz, with no ULVT. Each mapped run
checks eight crossing groups, four held-payload paths and eight local synchronizer
chains; minimum local synchronizer slack is 820.6ps in both configurations.
Restoring each database passes the RAM/load/boundary and hierarchy audits;
tightening the core clock is correctly rejected. No clock, crossing budget or
reset constraint was weakened to obtain these results.

The instance totals include one opaque accounting RAM leaf; its physical area,
power and intrinsic timing are excluded. FIFO storage remains synthesized logic.
Zero worst slack provides no extra margin. These are pre-layout logic and boundary
budget results, not routed timing, real-SRAM, reset recovery/removal, synchronizer
MTBF or workload-power signoff. Earlier RTL results below retain their own snapshot.

## 2026-11-16 multichannel coverage and synthesis diagnostics

The full eight-channel regression (31 tests × eight seeds) now has an accepted
coverage merge for the pre-cut RTL, unchanged from the checkpoint below.
Direct aggregation fails in the installed tool even with short input paths;
bounded native merges preserve all **248 original simulation identities** and
leave every input database byte unchanged. A separate 31-run comparison produces
identical direct/staged scores and metric numerators/denominators. Paths containing
spaces also pass. Native merged-test labels count intermediate batches, not the
original simulations; the provenance receipt records the latter.

A negative test mixing a two-channel run into the eight-channel design exposed
zero-exit instance-shape warnings. The merge now rejects native warnings as well
as errors; strict tool flags alone do not guarantee complete metric preservation.

| Eight-channel ordinary regression | Covered / expected | Coverage |
| --- | ---: | ---: |
| Declared PDMA scenario bins | 18 / 18 | 100% |
| Top-level line | 2588 / 2799 | 92.46% |
| Top-level condition | 1951 / 2956 | 66.00% |
| Top-level branch | 1241 / 1552 | 79.96% |
| Top-level toggle | 87346 / 906760 | 9.63% |

No metric exclusions were added. Generic VIP coverage remains in the report and
is not a PDMA feature-coverage claim. The earlier “64/64” scenario-bin statements
misread the tool's `AUTO BIN MAX` columns; those reports show 100% for the declared
scenario group, not 64 covered bins. They are corrected below and in the linked
review records. This finite covergroup is not exhaustive architecture coverage.

Both previous full synthesis runs timed out during optimization after mapping;
neither is a PPA pass. The flow now saves a source/library-bound mapped database
and critical-path report before optimization. Native fixture tests restore that
database and verify that diagnostic mode cannot claim final qualification.
The two-channel timing investigation and final eight-channel implementation
qualification remain open. Physical SRAM cost/timing remain excluded.

## 2026-11-13 multichannel checkpoint

The shared Copy/Fence implementation supports every integer channel count in 1..8.
This checkpoint is ready for source review, but **implementation qualification is
not complete**: 2/8-channel mapping is still running, and the aggregate coverage
report has not been accepted. See [ownership and lifecycle](../../docs/rtl/multichannel.md).

| Channel count | Positive DUT runs | Rejected checker mutations |
| --- | ---: | ---: |
| 1 | 31 | 8 |
| 2 | 31 | 8 |
| 3, 4, 5, 6, 7 (each) | 18 | 8 |
| 8 | 248 (31 tests, eight seeds) | 8 |

The 400 positive runs use independent task/Fence oracles, channel/context memory
permissions and per-channel lifecycle assertions. Tests cover peer progress while
owners are parked, retained write data after source changes, independent reset,
IRQ routing and post-global-reset reconfiguration. Eight-channel synchronizer
latency stress adds 18 positive runs and eight rejected mutations; five TB selftests
also pass. Native tests pass 20/20 with both compiler backends. The model's 26
page-fault tests and strict DMA Clippy check pass, including the 1..8 retention
characterization; production model behavior was not changed.

Ordinary zero-delay throughput remains 8 B/bus cycle on both read and write data
channels, with zero payload bubbles in the short, long, batch, clock-ratio and
multichannel cases. This is simulation throughput for the selected 64-bit profile;
it is not a claim that the new maximum configuration meets physical timing.

Three review findings have failing-first evidence and fixes: a new global reset
must win over an old channel-clear acknowledgement at the same edge; global reset
must invalidate all software channel configuration state; channel reconfiguration
must not modify the device notification mask. Global register operations now have
a device-level software owner, while channels retain their rings and local reset.

Fresh source-bound gates pass at 2 channels (CDC 66, RDC 2151) and 8 channels
(CDC 210, RDC 4076), with zero unresolved findings. Seven native review-gate checks
accept the exact reviewed input and reject stale identities, wrong/missing endpoints,
missing coverage, invalid grouping and wrong counts. Raw reports remain available;
review counts alone do not establish safety. No physical CDC/MTBF/reset signoff is
claimed.

The installed coverage tool fails while merging the complete 248-run input set
(`URG-LTFF`), including with explicit 64-bit execution. The singled-out database
loads successfully by itself. The complete input set has not been reduced or
reported as a successful merge. Investigation of merge behavior, final mapped
results and remaining physical-budget checks are the next work items. SRAM
area/power and intrinsic timing remain excluded from control-budget synthesis.


## Reusable-cell boundary cleanup (2026-11-12)

Testbench manifest SHA-256:
`0bc3503373ff97828293b02efd259063022a491ef193fcaffe4d8f1242e91ded`.
Passive checker names now belong to `common_cell`; DMA still owns the bindings.
Library documentation and constraint examples no longer prescribe DMA usage.

- Mailbox and FIFO binding qualifications each pass their positive case and reject
  all four injected violations with the expected diagnostics.
- Focused random-clock regression passes aligned/long Copy, read/write PageFault,
  channel reset and DMA reset at seed 11, and rejects all eight checker mutations.
- All nine package-resource tests pass. The 95 generated hardware declarations
  match the previous checkpoint; executable package constraint Tcl is unchanged.
- Fresh native CDC/RDC field comparisons are identical to the previous checkpoint.
  The stale review is rejected before rebinding. Final checks report 40/40/0 CDC
  and 1363/1363/0 RDC raw/reviewed/unresolved findings, without changing review rules.

This is focused qualification of naming/documentation changes. The complete
regression and synthesis results below retain their original input identities;
no new synthesis or physical-signoff result is claimed by this cleanup.

## Ownership and package-flow cleanup (2026-11-12)

Final testbench manifest SHA-256:
`3b383f6a9906f525d3b0bb11aefbb2e8f008d96256dd1bd3944a1c14a1bdeacb`.
See the [organization review](../../docs/rtl/organization-review.md) for findings,
responsibilities and the failing-before smoke correction.

- Target and random clocks each pass all 24 DUT tests at seeds 11/17; the separately
  instrumented latency snapshot passes all 24 at seeds 23/31. Each final batch
  rejects all eight checker mutations: **144 positive runs, 294 scenarios,
  318 terminal checks and 219,925 checked bytes**.
- All 17 DMA and 10 common-cell native tests pass with CC and Cranelift. The five
  TB selftests pass at seeds 11/17. Resource transport has nine passing tests;
  profile validation rejects 16 invalid configurations. Tool-cache integrity,
  log failure detection, runner failure aggregation and process cleanup tests pass.
- All four generated outputs are current. Six generator CLI/oracle-preparation
  tests, C layout checking and the native RAL and Rust-codec/SV oracles pass.
- The retained standalone SV smoke failed first on the obsolete one-outstanding
  reset assertion. Its final Copy/Fence, drain and post-reset traffic checks pass.
  Mailbox and FIFO binding qualifications each pass their positive case and four
  corruption/custody negatives.
- Native CDC/RDC fields are identical to the previous reviewed checkpoint: 40 and
  1363 findings, all explicitly reviewed, zero unresolved. The old identity was
  rejected before rebinding; all seven review-policy tests pass. Static sensitivity
  tests and the current register-mailbox formal boundary also pass.
- Fresh mapped integration passes instance isolation, wrong-depth, missing-path,
  impossible-budget and masked-path checks. The full mapping passes the original
  frequency targets with **0ps worst setup slack**, no ULVT, 74,120 instances and
  302 references. Cell area remains 32,267.907 in library units; all 40 synchronizer
  FFs are preserved. Eight crossing groups, four payload checks and eight local
  chains pass, with minimum crossing/local slack 211.3ps/820.6ps. Tightening the
  restored mapping's core clock is correctly rejected.

Only the 48 ordinary random-clock positive runs enter the final coverage merge.
The declared scenario group remains at 100%; module line/condition/branch coverage and
the zero-delay results are identical to the burst-window checkpoint below. Long
aligned payload remains 8B/bus-cycle with no gaps and 2160 descriptor-to-Fence
cycles; short, batched and half-core results remain 120, 519 and 2200 cycles.
There are no new metric exclusions or weakened timeout/diagnostic gates.

This qualifies the refactored organization and flows for the same minimal profile.
It does not add physical routing, hold/reset recovery/removal, MTBF, workload-power
or full DMA feature signoff.

## Burst-window qualification (2026-11-12)

Current testbench manifest SHA-256:
`648510316c892f13eab824bca945e6ae3b9e2a4d5d4f9852af6f25cc9168cd34`.
The production RTL and implementation inputs are bound by the current
[`dma-copy-review.tcl`](../../impl/dma/reviews/copy.tcl).
The simulation-only synchronizer-latency build has a separate manifest and cannot
be used by the synthesis/static flows.

All 24 DUT tests pass at target clocks and randomized asynchronous clocks, each
with seeds 11 and 17. The same 24 tests pass with independently injected 0/1-cycle
synchronizer delay at seeds 23 and 31. Together: **144 positive runs, 294 scenarios,
318 terminal checks and 219,925 checked bytes**. Throughput cases explicitly use
their zero-delay clock profile, including a half-core variant, in every batch;
other scenarios use the selected target/random clocks.
All three final batches also reject all eight mutations with their intended fatal
diagnostics and finish successfully; no timeout or launcher failure counts as a
correct rejection.

The final error checker permits only zero or the whole first page fragment in
the read-error scenario; an ordered write error must report the exact prefix.
It checks reported bytes and does not require an unchanged suffix. Native window
tests control the error/receipt ordering and verify the exact frozen snapshot.
The eighth checker mutation reports an impossible partial-fragment prefix.

One intermediate regression aggregation was interrupted by editing its live
shell script. Its logs remain separate and it is not an aggregate pass. The
launcher now parses its complete body before execution; the current final runs
use the stable launcher. Runner fixtures verify parallel overlap, eight negative
diagnostics, timeout rejection and cleanup of resistant descendants.

Coverage below merges only the **48 ordinary random-profile positive runs**.
It includes no injected RTL, negative tests or new exclusions. The scenario
covergroup reaches 100% of its declared 64 bins; this is not exhaustive feature,
parameter or protocol-error coverage.

| RTL module | Line | Condition | Branch |
| --- | ---: | ---: | ---: |
| DmaCopy | 100.00% | 86.21% | 100.00% |
| CopyEngine | 88.64% | 77.14% | 76.81% |
| CopyWindow | 97.20% | 81.95% | 95.16% |
| CopyMemoryEndpoint | 100.00% | 84.21% | 100.00% |
| CopyReadEndpoint | 100.00% | 80.70% | 100.00% |
| CopyWriteEndpoint | 100.00% | 83.12% | 100.00% |

Seventeen DMA native tests pass with both CC and Cranelift; ten common-cell tests
pass with both backends. These local tests are not merged into UVM coverage.
The W-before-AW fault case confirms actual data completion before its address
handshake and rejects premature Fence effects. Reset scenarios retain every
accepted write until its response, including multiple outstanding writes.
Performance, static review, constraints and mapping limits are recorded in the
[throughput review](../../docs/rtl/throughput-review.md).

## Testbench lifecycle cleanup (2026-11-11)

Testbench manifest SHA-256:
`9ec97793f8a1176f380b86821a6ac4aea012377a6417acd74063e7e6d42e4c5e`.
One fresh native build passes all **19 DUT tests x 8 random seeds** and all
**19 DUT tests with fixed clocks at seed 7**: **171 positive runs,
369 scenarios, 405 terminal checks and 32,045 checked bytes**.
Both batches reject all seven checker mutations with their intended diagnostics.
No positive DUT run reports a UVM error/fatal or assertion failure.

The ordinary DUT runs instantiate no clock-period selftest and log no
`CLOCK_CHECK` activity. Clock/reset-stimulus qualification is an explicit TB suite.
All **5 TB tests x 8 random seeds** pass; each also passes with fixed clocks in the
focused qualification. They test clock periods/reset edges, IRQ wait isolation,
timeout receiver cleanup, concurrent waits and reset-thread isolation. These
minimal fixtures do not exercise DMA task flows or contribute DUT coverage.

Before the fix, real native runs showed `IRQ_WAIT_SCOPE` after successful and
timed-out waits, and `RESET_WAIT_SCOPE` after reset: each operation killed an
unrelated caller-owned thread. Ordinary Copy and concurrent waits passed on that
same baseline. The corrected implementation preserves those threads and prevents
a timed-out receiver from consuming a later notification.

The parallel runner distinguishes DUT and TB suites, rejects unknown selection,
and excludes TB runs from DUT coverage. Runner tests also retain cancellation,
child-process teardown, timeout rejection and exact negative-diagnostic checks.
The coverage merge uses the build plus all 152 positive random DUT runs, excluding
negative and TB selftest databases. Native summary generation succeeds:

| RTL module | Line | Condition | Branch |
| --- | ---: | ---: | ---: |
| DmaCopy | 100.00% | 86.21% | 100.00% |
| CopyEngine | 93.09% | 78.07% | 85.90% |
| CopyMemoryEndpoint | 98.70% | 81.45% | 90.32% |

DUT RTL, AXI protocol checks and DUT invariant assertions are unchanged. This
qualifies the testbench lifecycle cleanup, not a new hardware feature, physical
signoff or measured wall-clock speedup. Raw logs and tool/deployment details stay
outside version control.


## UVM RAL qualification (2026-11-09)

Final testbench manifest SHA-256:
`2dcb7a6b8abf293c938cbf642d1a0ff07aa44a82bee53518ba0658851bfeb874`.
All **19 tests x 8 random seeds**, plus all **19 fixed-clock tests at seed 29**,
pass against this single fresh build: **171 positive runs, 369 scenarios,
405 terminal checks and 32,238 checked bytes**. Each batch also rejects all 7
checker mutations with the intended diagnostics. No positive run has a UVM
error/fatal or assertion failure. The external run watchdog is 300 seconds;
software polling and hardware reset/fault checks were not relaxed.

The new RAL scenario round-trips a 64-bit ring base through 32-bit AXI4-Lite, checks
that an unmapped read returns UVM_NOT_OK, restores configuration and completes
Copy/Fence. It first performs a real channel reset because disabling an already
configured channel does not unfreeze its ring configuration.

Separate real-UVM metadata tests cover 1/3 channels with 4/8-byte maps and 1020
channels across an aperture-alignment boundary, including relocation, fields,
access and reset metadata. A custom-schema oracle verifies valid zero parameters
and a source parameter named bus_bytes. Seven generator tests, one CLI test,
clippy, formatting and generated-output checks pass.

The initial RAL matrix exposed an existing directed write-fault fixture
assumption: one allocation allowed only 40 bytes before a page boundary, although
the test mutated all 64 source bytes. The final fixture aligns its 64-byte range
to 64 bytes so the full-source scratch-retention assertion is valid; no data
assertion was removed. General random/unaligned/page-crossing Copy tests remain.
The entire matrix was rerun after this correction.

Coverage summary uses all 152 positive random runs, with no metric/bin exclusions:

| RTL module | Line | Condition | Branch |
| --- | ---: | ---: | ---: |
| DmaCopy | 100.00% | 86.21% | 100.00% |
| CopyEngine | 93.09% | 78.07% | 85.90% |
| CopyMemoryEndpoint | 98.70% | 81.45% | 90.32% |

The scenario covergroup reaches 100% of its declared bins. Detailed report
generation crashes for this input set, including with 64-bit mode. Summary output
with the identical complete inputs succeeds. The flow uses text summaries and
retains raw databases; detailed report generation remains a tool limitation.
The executable tool selection was checked privately; no deployment paths are
included here. DUT RTL is unchanged, so this is register-model/verification
qualification, not a new synthesis or physical signoff result.

## Pre-RAL qualification (2026-11-09)

This checkpoint predates the generated UVM register-model migration. Its hardware
results remain useful history; it does not qualify the new register transport.

Testbench manifest SHA-256:
`6c43a423b3b7e68ac9cbe9d757740f6bb88e11eebed0adaae505b81283353547`.
One fresh build is shared by isolated test/seed runs; executable integrity is
checked before each launch. Raw logs and tool reports remain private.

- All **18 tests x 8 random seeds** pass: seeds 1, 7, 19, 41, 97, 211, 503, 997.
- All **18 tests** pass under the fixed-clock profile with seed **29**.
- Together these are **162 positive runs, 360 scenarios, 396 terminal checks**
  and **32,779 checked payload bytes**. No positive run has a UVM error/fatal
  or assertion failure.
- Both batches reject all **7 checker mutations** with their intended diagnostic.
  Early Fence publication through the IRQ error path and understated progress
  were incorrectly accepted before the checker fixes; both are now detected.
- The explicit event-boundary test checks a split error-event write, decoded
  identity/code/progress, exactly-once consumption and the untouched next slot.
- Runner fixtures cover parallel overlap, fixed-clock forwarding, input rejection,
  aggregate failure and child cleanup. A fatal message followed by a timeout is
  now a failed run, not an expected rejection; all real negative runs exited
  normally. Shell syntax, C11 layout checks and generated-record freshness pass.

The random-profile coverage merge includes all 144 positive runs, excluding
negative tests. The scenario covergroup reaches **100% of its declared bins**,
including actual allocation alignment and event-page crossing. This does not
mean exhaustive architecture coverage.

| RTL module | Line | Condition | Branch |
| --- | ---: | ---: | ---: |
| DmaCopy | 100.00% | 86.21% | 100.00% |
| CopyEngine | 93.09% | 78.07% | 85.90% |
| CopyMemoryEndpoint | 98.70% | 81.45% | 90.32% |

Combined HTML/text report generation crashed in the installed coverage tool.
The same complete input set succeeds with text output; the automated flow now
uses text reports without filtering metrics or adding exclusions.

The fixed profile has **33 successful Copy/Fence completions and 3,908 bytes**.
Submission-to-observation latency is **1.710 us minimum, 36.021 us weighted mean
and 103.160 us maximum**. This includes polling, injected delay and fault recovery;
it is not a throughput benchmark. Independent test randomization and allocation
placement differ from the earlier combined suite, so these cohorts do not form
a controlled performance comparison.

No DUT RTL changed in this cleanup. Native and synthesis evidence below remains
the earlier hardware baseline; those runs were not repeated. This qualification
is still limited to one-channel Linear Copy/Fence32/64, not other commands,
multichannel scheduling or physical CDC/RDC signoff.

## Independent-test checkpoint (2026-11-06)

All 17 independently selected tests have passing seed-1 results. The first
four-way run passed 16 tests and exposed the write-fault oracle's implicit
alignment assumption. That directed test mutates the source after a retained
burst read; its buffers now explicitly request bus-word alignment. All three
affected read/write/Fence fault tests were rebuilt and rerun successfully.
The manifests differ only in `tests/dut/fault.svh`:

- Initial manifest: `bcbb507ff7ac3c65a964c3241c304b921559b0cc8b43eb83c4f644aa660b6e6c`.
- Final manifest: `de2284e7d6c9c376b5aa61e835fe950629df045413472b3cf9b287bff698d75c`.

Five checker mutations are rejected with their required diagnostics. Runner
fixtures verify parallel overlap, aggregate failure, exact test markers, stale
output rejection, positive-only coverage inputs and child-process cleanup on
cancellation. Actual execution also caught and fixed the older Bash empty-array
behavior in the runner. Shell syntax, C11 layout checks and flow selftests pass.

The schema-backed SV generator has four passing Rust tests, reproducible output
and a simulator oracle comparing complete Copy/Fence byte images and Event
decoding against independent Rust codecs. The allocator dependency patch has
three failing-before scenarios and four passing-after scenarios; external
checkouts remain unchanged.

This is a pause checkpoint, not a new full coverage or performance qualification.
The complete multi-seed matrix and coverage merge have not been repeated after
the final ownership changes. RTL is unchanged from the native/synthesis baseline
described below.

## Earlier combined-suite baseline

The following measurements describe the earlier combined suite, before the
test split. They are retained for provenance rather than copied to the new runs.

Validated on 2026-11-06 after the testbench refactor and minimal Fence extension.
The sorted testbench input manifest has SHA-256
`47096cfb8cee0634e096a0214e858c286fbef71b8b135cc5ea0a12b990b5801f`.
RTL and testbench source manifests, complete logs, coverage databases and
implementation reports are retained with the local run artifacts.

## Observed results

- Random clock profiles: seeds **1, 7, 19, 41, 97, 211, 503, 997** pass.
- Fixed clock profile: seed **29** passes with `+clock_mode=fixed`.
- Each run completes 39 scenarios: **351 scenarios**, **387 terminal task checks**
  and **32,989 completed-task payload bytes** across all nine runs.
- All positive runs have zero UVM errors/fatals and no assertion failures.
- Five negative tests fail with the intended diagnostic: payload corruption
  (`TASK_DATA`), wrong and unknown identity (`TASK_IDENTITY`), omitted Fence
  (`TASK_TIMEOUT`), and duplicate terminal result (`TASK_DUPLICATE`).
- Native Veryl tests pass **13/13** with CC and **13/13** with Cranelift.
  New Fence tests cover scalar widths, lane guards, ordering behind a delayed
  Copy receipt, fault/resume, permanent write error, reset drain and invalid
  alignment/range. Failing-first Fence and word-strobe proofs were captured.
- The independent SV smoke, C11 contract-layout checks, shell flow selftest and
  mapped synthesis pass.

Successful flows poll a real Fence marker, explicitly check the task result and
memory, then drain reset before freeing their ring/payload resources. Software
reset remains in the scenario. Only power-on reset uses UVM `reset_phase`.
Three independently configured jarvisuk clock groups pass period and synchronous
reset-release checks. Handler and checker instances are channel-specific; the
single implemented channel uses platform vector 17.

## Coverage and performance

The scenario covergroup reaches **100% of its declared bins** for outcomes,
length classes, alignment, page-boundary cases and Fence width. The merged
coverage report includes the eight random-profile positive runs only, excluding
negative tests. This is not exhaustive architectural coverage.

| RTL module | Line | Condition | Branch |
| --- | ---: | ---: | ---: |
| DmaCopy | 100.00% | 86.21% | 100.00% |
| CopyEngine | 92.55% | 77.54% | 84.62% |
| CopyMemoryEndpoint | 98.70% | 81.45% | 90.32% |

Local native tests and this scenario suite have different purposes; native
coverage is not merged into these simulator percentages. Protocol-error paths,
unexercised guards and constant-width/profile bits remain visible in the report.
No exclusions were added to improve the score.

The fixed profile records 33 successful Copy/Fence completions and 3,576 bytes.
Submission-to-observation latency is **1.700 us minimum, 26.751 us mean and
100.360 us maximum**. It includes software polling, asynchronous clocks, injected
delays and recovery cases. These mixed small scenarios are a latency diagnostic,
not a peak-throughput benchmark. The passive subscriber reports byte counts and
latency without owning task resources or affecting stimulus.

This remains a one-channel Linear Copy/Fence32/64 RTL profile. The organization
supports channel-specific verification ownership, but does not establish
multichannel, other-opcode or analog CDC/RDC signoff. See
[requirements](requirements.md) for scenario mapping and
[dependencies](dependencies.md) for the platform/library boundaries.

## Future scope

Concurrent task/result routing and additional DUT features need their own
scenarios; the current software flow intentionally handles one task per channel
at a time. SV record and UVM register generation now use the published schema
backends; the older contract-local generator checkpoints above are historical.
