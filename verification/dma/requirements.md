# DMA RTL verification obligations

The DUT is the current one-channel `DmaCopy` profile, with AXI4-Lite 32-bit
register access and a 64-bit memory port using the ordinary AXI subset of AXI5.
Atomic traffic is forbidden in this profile. The bench uses the actual protocol
VIP for both ports. Test software publishes descriptors through jarvisuk memory
and software-owned MMIO sequences. Copy followed by Fence32/64 produces a real
memory marker that software polls for successful completion. Destination equality
never signals completion. Fault, error and fatal observations use registers and
error events through the same software channel abstraction. Register operations
use generated UVM register handles and the protocol VIP's native RAL adapter;
software never substitutes mirrored values for a real hardware read.

| Obligation | Model reference under `crates/scenarios/tests` | RTL scenario/check |
| --- | --- | --- |
| Linear copy, independent endpoint selectors | `single_channel/cases/linear.rs` | Random bytes, lengths, alignments, nonzero endpoint offsets and selectors; completed-task data and guards; platform memory permissions enforce access ranges and contexts |
| Byte tails and page boundaries | `single_channel/cases/linear.rs` | Byte/full-width transfers; no access crosses 4 KiB; no over-read/write |
| Software-visible successful completion | `single_channel/software/mod.rs` | Copy followed by Fence32/64; poll the marker, then check data, guards and identity before task buffer release |
| Generated register frontdoor | Contract register schemas and MMIO bus contract | Complete reset before changing frozen configuration, round-trip a 64-bit base through AXI4-Lite32, propagate an unmapped read error, restore configuration and complete Copy/Fence |
| Recoverable read/write faults | `single_channel/recovery` | Park with no error event/IRQ; no retry before Resume; successful read scratch retained across failed writes |
| Stopping errors | `single_channel/cases/faults.rs` | Error at the next payload page; event identity/code/confirmed prefix and prefix memory; successor Fence remains unpublished |
| Ring progress and reuse | `single_channel/software` | Copy/Fence pairs reuse a four-slot ring without reset; consumer progression and correct descriptor slot |
| Descriptor decode and zero-length Copy | `dma/tests/edges.rs`, `dma/src/channel/command.rs` (under `crates`) | Malformed encoding is CommandRead fatal, valid unsupported opcode is an Unsupported event, zero length succeeds without payload/event |
| Reporting failures | `single_channel/cases/faults.rs` | Failed event writes retain fatal and do not publish producer |
| Event publication across a page boundary | `single_channel/cases/faults.rs` | Explicit channel-owned event placement; decode identity, error code and exact progress after the split write; neighboring ring slot remains intact |
| Reset while accepted memory access is outstanding | `single_channel/harness/lifecycle` | Channel and global reset with suspended VIP B response; reset busy stays set until response is released; no later chunk write; fresh copy after reset |
| Notification acceptance versus software dispatch | `single_channel/hardware/tests.rs` | Backpressure at IRQ output; exactly one accepted notification dispatched through jarvisuk IRQ service to the channel's own handler; channel zero uses vector 17; ISR reads cause through VIP MMIO; stopping errors bypass routine masks; enable never replays |
| Three asynchronous domains | RTL-specific | Independently randomized periods, phases and bounded jitter; cold reset released on each domain's own edge |
| Checker sensitivity | TB selftest | Corrupt payload (`TASK_DATA`), wrong or unknown identity (`TASK_IDENTITY`), suppress completion (`TASK_TIMEOUT`), duplicate completion (`TASK_DUPLICATE`), publish an early Fence during IRQ error (`TASK_COMPLETION`), or understate progress (`TASK_ERROR_INFO`); require the corresponding fatal diagnostic |

These are scenario translations, not assertions that RTL must follow the Rust
model's tick sequence. Scenarios orchestrate task flows; software channel and IRQ
objects own MMIO. Physical power-on reset runs only in `reset_phase`, while
software reset is part of the scenario. A resource-owning task flow allocates,
submits, waits for completion and explicitly calls the task checker before freeing
its payload and Fence buffers. Error cleanup completes software reset drain first.
Channels own their command/event rings across tasks. A reset-cancellation flow
waits for drain, checks the reset result, then frees its resources. Each channel
owns separate software, IRQ-handler and checker instances; the current DUT has
only one channel, so this organization does not establish multichannel coverage.
The checker derives expected bytes and identity from submitted software inputs.
Directed error scenarios derive the expected committed prefix from the injected
failure address, independently of the event's reported count. Fault/error checks
read Fence storage even when the result arrived through the interrupt handler.
Passive subscribers receive channel-tagged scenario outcomes for coverage and
submission-to-observation latency. They neither inspect payload transactions nor
own task resources. Polling rejects missing completion; the checker rejects
duplicate terminal results, data and identity mismatches. Memory storage is one
`svt_mem` instance, shared by the VIP's native responder and a jarvisuk
`jvs_memory_model` adapter. jarvisuk owns allocation/block access, not a shadow
bus executor. Failed writes may have affected memory; replay checks must not
assume transactional rollback.

Fault injection is a platform response policy. Payload SLVERR plus USER=1 means
recoverable PageFault; other errors stop. This does not model page tables or PCIe.

The initial profile implements Linear Copy and Fence32/64. It does not implement
SG, multi-channel arbitration, Write, Fill, Wait, Atomic, Semaphore, Trap or
Timestamp. Those Rust scenarios are future
coverage obligations, not passed/skipped tests in this regression. Protocol VIP
and randomized clocks do not establish analog CDC/RDC or metastability signoff.

Record each run's seed, source hashes, tool identity, scenario/task counts,
functional and code coverage. A pass marker alone is insufficient: reject UVM
errors/fatals, timeouts, missing test completion and vacuous activity. Coverage
reports combine only successful seeds, excluding checker mutation runs.

## Pending testbench lifecycle cleanup

The following review decisions are approved follow-up work. They are not yet
implemented; this checkpoint changes documentation only.

- Ordinary DUT regressions must not run the clock-generator selftest. The current
  `pdma_clocks` checks 64 periods per domain, rather than sampling periods for the
  entire simulation, but repeats that work and holds a `run_phase` objection in
  every DUT case. Move period/jitter and reset-stimulus edge qualification into
  explicitly selected TB selftests. The ordinary clock component retains stimulus
  configuration, power-on reset control, completion waits and bounded watchdogs.
  Remove the unconditional `clocks_verified == 3` requirement from the DUT base test
  together with that move; retain equivalent non-vacuity checks in the TB selftest.
- `pdma_clocks` is a `uvm_component` around the existing `jvs_clk_env`, not a second
  independent test environment. Clarify its naming and keep its responsibility
  limited to clock/reset integration. Selftest observers must be absent from
  ordinary runs, rather than waking each cycle to test an enable flag.
- Use one task-phase family within each owned component. Currently `pdma_clocks`
  mixes `reset_phase` and `run_phase`, and `pdma_env` mixes `configure_phase` and
  `run_phase`. Keep physical reset in `reset_phase`; use the runtime phase sequence
  for lifecycle-managed services, including IRQ registration and service execution.
  A continuously passive monitor may use only `run_phase`. Do not mechanically
  rewrite third-party VIP phase implementations.
- Bound cancellation to the operation that owns the competing processes. The
  current `disable fork` sites in `await_reset`, clock phase tasks and
  `wait_interrupt` need review/removal or an isolated calling process. A normal
  task call does not create a process boundary: cancellation can affect earlier
  children of its caller, not merely the adjacent lexical fork. Other components'
  independent phase processes are not affected solely by sharing a phase. Use
  per-invocation process isolation or explicit process handles; a named block
  alone is not a general solution for concurrent/reentrant calls.

Keep AXI protocol checks, DUT invariant assertions and IRQ handshake observation
in ordinary verification. These check DUT behavior, not the clock generator.
IRQ monitoring must preserve consecutive accepted notifications even when the
notification signal stays high. Scenario coverage already samples completed
observations, rather than every clock cycle. Software MMIO polling and cycle-based
timeouts have behavioral meaning and must not be removed just to reduce activity.

Before changing the implementation, capture a failing process-isolation test with
an unrelated caller-owned thread. Cover successful IRQ waits, timeout cleanup and
concurrent invocations. Qualify the clock selftest independently, then run the
existing DUT scenarios and checker-negative cases with the selftest disabled.
Record any runtime comparison under matching configuration; no performance gain
has been measured at this checkpoint.
