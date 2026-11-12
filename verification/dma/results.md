# Scenario-level verification checkpoints

## Burst-window qualification (2026-11-12)

Current testbench manifest SHA-256:
`648510316c892f13eab824bca945e6ae3b9e2a4d5d4f9852af6f25cc9168cd34`.
The production RTL and implementation inputs are bound by the current
[`dma-copy-review.tcl`](../../impl/pdma/constraints/dma-copy-review.tcl).
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
