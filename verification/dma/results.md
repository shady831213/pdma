# Scenario-level verification checkpoints

The baseline below predates the independent-test split, persistent channel-owned
rings and schema-generated SV records. Its coverage and timing figures do not
qualify those newer changes.

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

## Next review and qualification

- Run the full test-by-seed matrix and fixed-clock profile after the latest
  ownership changes, then regenerate coverage and performance summaries.
- Review remaining numeric error expectations and register masks against
  generated symbols, and separate platform/test configuration from wire constants.
- Add an explicit event-ring page-boundary scenario: independent natural ring
  allocation no longer guarantees the old slab's boundary placement.
- Review channel/ring lifetime and software/checker boundaries together before
  expanding randomized concurrent scenarios or the DUT feature set.
- Consider a generated UVM register model separately; the current record package
  and RTL layout packages do not claim that capability.
