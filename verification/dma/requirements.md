# DMA RTL verification obligations

The DUT is `DmaCopy`, with 1..8 channels and a default AXI4-Lite 32-bit register
port / 64-bit ordinary memory port. The testbench obtains channel count, data
widths and resource dimensions from the same immutable integration profile as
the DUT; see [static configuration](../../docs/rtl/configuration.md).
Atomic traffic is forbidden in this profile. The bench uses the actual protocol
VIP for both ports. Test software publishes descriptors through jarvisuk memory
and software-owned MMIO sequences. Copy followed by Fence32/64 produces a real
memory marker that software polls for successful completion. Destination equality
never signals completion. Fault, error and fatal observations use registers and
error events through the same software channel abstraction. Register operations
use generated UVM register handles and the protocol VIP's RAL adapter with
AXI4-Lite lane handling for 32-bit registers on a 64-bit bus;
software never substitutes mirrored values for a real hardware read.

| Obligation | Model reference under `crates/scenarios/tests` | RTL scenario/check |
| --- | --- | --- |
| Linear copy, independent endpoint selectors | `single_channel/cases/linear.rs` | Random bytes, lengths, alignments, nonzero endpoint offsets and selectors; completed-task data and guards; platform memory permissions enforce access ranges and contexts |
| Byte tails and page boundaries | `single_channel/cases/linear.rs` | Byte/full-width transfers; no access crosses 4 KiB; no over-read/write |
| Write32/64 | `dma/tests/command_operations.rs` | Write a random immediate to an aligned target in its selected context. Check the exact bytes and guards after the successor Fence. |
| Fill | `dma/tests/command_operations.rs` | Compare each byte to the 1/4/8-byte pattern from the command start, including page splits and tails. Park two commands with different patterns and resume them in reverse order. Make the peer Fill longer than its reserved window so the checker observes both retained bytes and bytes generated from the restored command. Check a permanent write error and reset while a B response is outstanding. Measure aligned throughput with no payload reads. |
| Timestamp | `firmware_managed/harness/runtime/tests/copy_engine/timestamp.rs`, `dma/tests/command_operations/faults/timestamp.rs` | Bracket a faulted Fill with two Timestamp commands. Check marker order, sample bounds and Fence completion. Native execution tests retry a failed Timestamp write after changing the external timer and require the original sample. |
| Software-visible successful completion | `single_channel/software/mod.rs` | Copy followed by Fence32/64; poll the marker, then check data, guards and identity before task buffer release |
| Trap after Fence | `single_channel/recovery/mod.rs` | Publish Copy, Fence and Trap together. Check the marker and copied bytes on the channel interrupt, sticky cause and ACK, assertion on unmask/enable, coalescing while sticky, and deassertion on ACK or reset. |
| Generated register frontdoor | Contract register schemas and MMIO bus contract | Complete reset before changing frozen configuration, round-trip a 64-bit base through AXI4-Lite32, propagate an unmapped read error, restore configuration and complete Copy/Fence |
| Recoverable read/write faults | `single_channel/recovery` | Park with no error event/IRQ; no retry before Resume; successful read scratch retained across failed writes |
| Stopping errors | `single_channel/cases/faults.rs` | Error at the next payload page; event identity/code/confirmed prefix and prefix memory; successor Fence remains unpublished |
| Ring progress and reuse | `single_channel/software` | Copy/Fence pairs reuse a four-slot ring without reset; consumer progression and correct descriptor slot |
| Descriptor decode and zero-length Copy | `dma/tests/edges.rs`, `dma/src/channel/command.rs` (under `crates`) | Malformed encoding is CommandRead fatal, valid unsupported opcode is an Unsupported event, zero length succeeds without payload/event |
| Independent SG endpoints | `dma/tests/scatter_gather.rs`, `dma/tests/scatter_gather_faults.rs` | Correct logical bytes across unequal physical splits, validation before payload, no access to unused list entries, retained write data on Resume, and peer progress during recovery |
| SG span ownership during a fault | `dma/tests/scatter_gather_faults.rs` | Park on the first span with unequal source and destination lengths. A peer completes a segmented Copy. On Resume the owner writes its retained bytes, walks its remaining spans and completes its Fence. Exercise each physical channel as owner. |
| SG error prefix across spans | `dma/tests/scatter_gather_faults/payload.rs` | A read error at the start of the second span reports the first span's bytes. A write error at the next page boundary includes both the first span and acknowledged writes in the second. Check the event and the logical destination bytes on each channel. |
| Command ring admission | `dma/tests/edges.rs` | Each channel rejects enable with zero, non-power-of-two or oversized capacity, a misaligned base, or an overflowing ring extent. Software reads back disabled and reprograms the ring for a checked Copy and Fence. |
| Channel reset with a parked burst window | `single_channel/recovery` | Fault a seed-selected read or write burst after one window of transfers. On each physical channel, reset the parked command and verify a fresh Copy and its Fence after the same channel is reconfigured. |
| SG address validation | `dma/src/channel/work/cursor.rs`, `dma/src/channel/work/validation.rs` | `pdma_sg_list_address_test` denies list and payload reads and checks the event for an exhausted index, overflowing index/stride, overflowing table address or end, and misalignment. `pdma_sg_range_test` checks the used payload interval, including empty entries and clipped tails. |
| Reporting failures | `single_channel/cases/faults.rs` | Failed event writes retain channel fatal and leave the producer unchanged; observe the reporting failure through the channel ISR |
| Event publication across a page boundary | `single_channel/cases/faults.rs` | Explicit channel-owned event placement; decode identity, error code and exact progress after the split write; neighboring ring slot remains intact |
| Reset while accepted memory access is outstanding | `single_channel/harness/lifecycle` | Channel and global reset with suspended VIP B response; reset busy stays set until response is released; no later chunk write; fresh copy after reset |
| Level IRQ and software dispatch | `single_channel/hardware/tests.rs` | Sample each channel independently through the stock JarvisUK level monitor. Verify held causes, source masking, diagnostic publication and independent ISR service over shared MMIO. |
| Three asynchronous domains | RTL-specific | Independently randomized periods, phases and bounded jitter; cold reset released on each domain's own edge |
| Checker sensitivity | TB selftest | Corrupt payload (`TASK_DATA`), wrong or unknown identity (`TASK_IDENTITY`), suppress completion (`TASK_TIMEOUT`), duplicate completion (`TASK_DUPLICATE`), publish an early Fence during IRQ error or channel fatal (`TASK_COMPLETION`), understate ordered-write progress or report an impossible partial read-error prefix (`TASK_ERROR_INFO`); require the corresponding fatal diagnostic |

These are scenario translations, not assertions that RTL must follow the Rust
model's tick sequence. Scenarios orchestrate task flows; the software device
and channel drivers own MMIO. Physical power-on reset runs only in `reset_phase`, while
software reset is part of the scenario. A resource-owning task flow allocates,
submits, waits for completion and explicitly calls the task checker before freeing
its payload and Fence buffers. A task that has observed its own Fence can release
its buffers while a later command on the same channel is stopped. The command
consumer reports descriptor acceptance. The channel retains its command and
event storage. For a stopped task the scenario explicitly resets the channel
before releasing buffers. A reset-cancellation flow
waits for drain, checks the reset result, then frees its resources. The software
device owns the channel drivers. Each channel environment uses its corresponding
driver and owns its IRQ service and task checker. Multichannel scenarios
check independent notifications and addressable state on the configured channels.
The checker derives expected bytes and identity from submitted software inputs.
Directed error scenarios derive the expected committed prefix from the injected
failure address, independently of the event's reported count. In the read-error
case, the first valid page fragment is one complete bus beat: its write may or
may not finish before the later read error, so only zero or that whole fragment
is permitted. The ordered write-error case requires the exact preceding prefix.
Native window tests control receipt ordering and check the exact error-time
snapshot. No error test assumes that bytes beyond the prefix remain unchanged.
Fault, stopping-error and channel-fatal checks read Fence storage directly and
reject writes by the incomplete task's successor. Fence and error events supply
task identity. Fault and channel-fatal registers identify only the channel.
Passive subscribers receive channel-tagged scenario outcomes for coverage and
submission-to-observation latency. They neither inspect payload transactions nor
own task resources. Polling rejects missing completion; the checker rejects
duplicate terminal results, data and identity mismatches. Memory storage is one
`svt_mem` instance, shared by the VIP's native responder and a jarvisuk
`jvs_memory_model` adapter. jarvisuk owns allocation/block access, not a shadow
bus executor. Permanent failed writes may have affected memory. A repairable
PageFault write, in contrast, must have no effects from that failed access before replay.

Fault injection is a platform response policy. Payload SLVERR plus USER=1 means
recoverable PageFault; other errors stop. This does not model page tables or PCIe.

The RTL implements multichannel linear and scatter/gather Copy, Write, Fill,
Timestamp, Fence32/64 and Trap. Wait, Atomic and Semaphore remain future RTL
coverage obligations, not passed/skipped tests in this regression. Protocol VIP
and randomized clocks do not establish CDC/RDC or metastability signoff. VC
SpyGlass CDC and RDC are separate required checks on the actual emitted DMA
design; their constraints, negative fixtures and closure obligations are specified
in [reset/CDC](../../docs/rtl/reset-cdc.md#vc-spyglass-cdcrdc-检查门槛).

Record each run's seed, source hashes, tool identity, scenario/task counts,
functional and code coverage. A pass marker alone is insufficient: reject UVM
errors/fatals, timeouts, missing test completion and vacuous activity. Coverage
reports combine only successful seeds, excluding checker mutation runs.

### Whole-DMA coverage review

Review the implementation instance tree for each elaborated profile and sample
software-visible behavior at task and channel boundaries. The following crosses
guide constrained-random traffic and directed corner cases. Each needs observed
evidence before it can be counted as covered.

| Owner | Behaviors to distinguish |
| --- | --- |
| Register frontend and control | Access size and byte enables, independent AW/W arrival, channel selection, enable and mask changes with a pending cause, W1C, and channel versus global reset |
| Command rings, fetch and grant | Empty/full, pointer wrap and refill, competing ready channels, delayed descriptor returns, and reset of a channel with accepted fetches |
| Execution and retained data | Linear/SG endpoints, unequal boundaries, Fence and Trap ordering, payload read/write fault, Resume/reset, and completion by a peer while the owner is parked |
| Transport and memory endpoint | Backpressure at issue and return, independent AW/W stalls, same-ID order, cross-ID completions, interleaved read beats, and reset with outstanding debt |
| End-to-end software contract | Confirm payload and Fence memory, event contents and committed prefix, per-channel IRQ cause/level, reset drain, and resource reuse |

Sample actual outcomes in the functional coverage model. A randomized setting
for reorder, for example, is evidence of stimulus configuration; a returned
younger transaction observed ahead of an older transaction is evidence of
reordering. Attribute structural holes to the responsible instance and keep
constant or unreachable bins visible with their rationale.

## Throughput and bubble qualification

### Multichannel qualification

The RTL slice supports every integer channel count from one through eight, with eight as the
initial supported maximum. Use two channels for routine development, simulation and
synthesis. Retain one-channel regressions and a four-channel scaling check.
Final acceptance requires the eight-channel configuration to close functional
and mixed-traffic scenarios, independent reset, retained-fault resume, shared
resource pressure, randomized clocks/backpressure, throughput, CDC/RDC and
synthesis timing. Smaller configurations or elaboration alone cannot substitute.
Counts 3/5/6/7 also require functional, index-wrap, arbitration and independent
reset checks; they are not inferred to work from the power-of-two configurations.

Resource checks follow the model's ownership and progress contract: parked read
data stays intact; reset does not release resources while old work can still use
them; unrelated healthy channels retain progress when their external accesses can
complete. A shared pool must not silently weaken those guarantees. New observable
admission/capacity behavior requires explicit specification and model tests before
RTL qualification. Tests compare task effects and lifecycle guarantees, not equal
physical slot counts, chunk boundaries or cycle traces between model and RTL.

### Datapath measurements

The default eight-slot window and asynchronous request/receipt queues are measured
in [the throughput review](../../docs/rtl/throughput-review.md). The requirements
below remain acceptance criteria for further configurations, not a declaration
that every alignment, width and clock ratio is closed.

These requirements were agreed on 2026-11-11; implemented cases and their qualified
configurations are recorded in [results](results.md). Reference the existing Veryl tests and UVM zero-delay
case construction in [pcie_modeling](https://github.com/shady831213/pcie_modeling),
adapting their methodology to the DMA contract rather than importing project-specific
paths, deployment settings or assumptions.

- Native Veryl tests cover continuous offers/acceptance, local stage and buffer
  handoffs, simultaneous release/replacement, and backpressure recovery. Check
  sustained initiation rate and locate avoidable idle cycles.
- Separate UVM zero-delay scenarios use the real VIP/memory path, continuous
  legal acceptance and minimum legal response latency, with no artificial responder
  waits. Zero-delay does not mean zero CDC latency or a protocol-violating combinational
  response. Keep each case independently runnable by the parallel runner.
- Exercise long aligned copies, back-to-back tasks and short tasks. Measure both
  the steady data phase and full submission-to-Fence completion; expose fill/drain,
  descriptor fetch and task-boundary costs rather than hiding them outside the window.
- Use the external bandwidth reference from the architecture, not the DUT's current
  peak as its own target. Classify bubbles by causal evidence. Real dependencies and
  external pressure differ from periodic gaps caused by undersized internal resources
  or repeatedly restarting control; the latter must be fixed or explicitly justified
  against the agreed capacity/throughput requirement.
- Retain three genuinely independent clock domains. Cover the target frequencies
  with differing phase, plus representative clock ratios and controlled backpressure;
  equal core/bus frequency is not a synchronous-clock assumption.
- Functional checks stay at scenario/task scope, using real Fence completion and
  independently expected memory/state. Passive bus observations provide throughput
  metrics; VIP and reusable assertions check protocol, credit/custody and no early
  completion. Do not introduce a shadow hardware executor.
- Increasing in-flight work must retain reset drain, fault/park/resume and error
  reporting semantics. Recheck CDC/RDC and scoped timing constraints after changing
  transport; existing mailbox waivers do not automatically cover a FIFO implementation.

## Testbench lifecycle and selftest scope

Ordinary DUT regressions do not instantiate the clock-generator selftest.
`pdma_clocks` in `env/clocks.svh` is a component around the existing `jvs_clk_env`;
it owns configuration, power-on reset control, event-driven completion waits and
a bounded reset watchdog. It has no `run_phase`, period sampler or selftest counters.
All three real reset interfaces must finish before `reset_phase` is released.

Explicit TB selftests own period/jitter and reset-stimulus edge qualification.
They check 64 periods per domain against the requested configuration, independently
of the driver's period-conversion helpers. Only that selftest retains the required
sample counts; DUT test completion depends on scenarios and task checks. No disabled
observer wakes each cycle to check an enable flag.

Each source registers its driver on local pin 0 in `configure_phase`. The JarvisUK level monitor
samples the corresponding wire on core clock edges and rearms after the handler
returns. A blocked channel handler can share the RAL frontdoor with peer handlers.
Trap handlers clear the sticky bit by MMIO. Stopping-error handlers service the
event and disable channel IRQ until reset. Device fatal has a separate handler and
enable. The clock component uses `reset_phase` and its selftest adds `post_reset_phase`.

JarvisUK queues each ISR's observation after MMIO service completes. Channel
error and device fatal IRQ tests wait on that result directly. For a stopping
error the channel ISR can receive the interrupt before the event write finishes.
It waits for a diagnostic or fatal status when the diagnostic is still outstanding.
The scenario consumes it with `get_irq_result`. The testbench supplies its wait
budget in simulation nanoseconds, derived from the nominal core period; the
library polls its FIFO at 100 ns intervals. The IRQ selftests check isolation of
concurrent waits, survival of a caller-owned thread and delivery of a notification
that arrives after timeout. The power-on reset watchdog has a separate fork owner
so cancelling it affects only that reset operation.

AXI protocol checks, DUT invariant assertions and level monitors remain active in
ordinary verification. Scenario coverage samples completed observations.

`PDMA_SUITE=dut` is the default regression suite, including the nine checker
mutation cases. `PDMA_SUITE=tb-selftest` selects the independent cases listed in
`flow/tb-selftests.list`; they do not run DMA task flows or contribute DUT coverage.
The coverage flow rejects that suite. The same bounded parallel runner and
per-case process-group cleanup serve both suites.

`PDMA_CLOCK_MODE=target` uses 2ns/1ns/1ns reg/core/bus periods with the configured
phase offsets. It does not replace randomized asynchronous ratios/jitter or imply
related clocks in static analysis. Both DUT and TB selftests use the same
test-owned clock-mode selection; period checking remains TB-selftest-only.

Record runtime comparisons only with matching inputs and configuration. This
cleanup removes redundant selftest work from DUT runs; no wall-clock speedup is
claimed from the reduced clock checks.
