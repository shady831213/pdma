# Copy-engine commands and error events

Status: implemented and model-verified, 2026-10-22. This chapter
specifies the replacement copy-engine protocol. The ABI is not frozen. The
[roadmap](firmware-dma-goal.md#agreed-next-feature-set-not-yet-implemented)
records the motivation, reference evidence and excluded features.

## Ownership and execution

**HOST-CE-1.** Software allocates command, event, payload, dependency and completion
storage, publishes commands, and retains every referenced resource until qualified
completion or acknowledged cancellation. A fetched command slot becoming reusable
does not imply that its referenced resources can be freed. Context lifetime rules
in [context-contract.md](context-contract.md) apply to new operations as well as Copy.

**HW-CE-1.** DMA executes commands through its existing shared, step-based pipeline.
Same-channel successors cannot pass a waiting, suspended or failed command. A false
Wait/SEM condition releases the shared execution grant so another channel can
satisfy the dependency. Accepted ownership survives this release. Ordinary waiting
does not set FAULT_STATUS or require RESUME.

**FW-CE-1.** Firmware continues to manage grants, contexts and lifecycle operations.
It does not submit every VF command, implement DMA dependency polling, or proxy
ordinary channel error consumption. PF management command/response/event rings
and DMA-wide fatal routing retain their existing contracts.

**PLAT-CE-1.** Peripheral adapters authorize every access against the channel owner
and selected context, including atomic access and completion publication.
They preserve platform source tags and downstream failures. Runtime selects memory
backing and delivers notifications; it owns no command execution or semaphore policy.

## Command meanings

| Command | Effect |
| --- | --- |
| Copy | Preserve independent Linear/SG endpoints and existing retained fault checkpoints. |
| Write | Write immediate data to an authorized target. A memory context is not permission to modify privileged registers. |
| Fill | Repeat a pattern over a destination range without reading a source buffer. |
| Wait | Observe a memory condition without modifying its value; wait until satisfied. |
| Atomic | Perform one indivisible read-modify-write using the specified operation and memory ordering. |
| SEM Signal | Atomically increase the permit count. |
| SEM Wait | Atomically consume one permit when the count is positive; otherwise wait without modification. |
| Fence | Complete prior operations in its specified scope, then publish a software-selected memory value. |
| Trap | Request a channel notification; no success event is appended. |
| Timestamp | Sample the external platform timer once and write its 64-bit value to an authorized target. |

Timestamp units and progression are platform-defined, not DMA clock cycles.
No generic instruction branching, indirect command buffer, batch container, GPU
page-table operation, cache-maintenance command or AQL compatibility is introduced.

## Wire layout

The fixed Command slot is 96 bytes with alignment eight. Each complete command
starts at slot offset zero and contains the same one-byte `CommandHeader` opcode
and an ID at offset eight. Opcode values 0 through 8 select Copy, Write, Fill,
Wait, Atomic, Semaphore, Fence, Trap and Timestamp respectively. The opcode enum
is defined once. It is not inferred from singleton enums in the individual cases.

The generic untagged union only overlays complete records. Its explicit case
codec knows nothing about opcodes. PDMA's contract reads the common header and
chooses a case; protocol encoding rejects a semantic case whose header names a
different operation. Copy's endpoint unions remain tagged unions, selected by
its own options fields. This separates storage layout from protocol selection.
Schema declarations spell those fields `tagged_union { tag: ..., cases: ... }`.
The independent untagged layout uses `union_schema! { ... union ... }`; the old
inline `union { tag: ... }` spelling is rejected rather than retained as an alias.
`codec: external` optionally supplies mechanical `Encode`/`Decode`, convenience
methods and normal view adapters. They call the contract's direct `SchemaCodec`
implementation; the option does not generate that implementation. Schema receives
no opcode map, prefix or selection rule. Pure unions
without this option need no protocol codec or selector. Their `variant` and
`set_variant` view helpers make the chosen storage interpretation explicit.
For Command, normal `values`/`set_values` views use the external protocol checks,
including rejecting an inconsistent opcode before mutation.

| Complete command | Bytes before unused slot tail | Operands after header/ID |
| --- | ---: | --- |
| Copy | 96 | Options at 1, SG context at 4, length at 16, payload contexts at 24/28, source/destination unions at 32/64 |
| Write / Fence | 40 | WordWrite at 16 |
| Fill | 48 | Address/length/pattern/context/width at 16/24/32/40/44 |
| Wait | 48 | WordTarget/value/comparison at 16/32/40 |
| Atomic | 56 | WordTarget/order/operation/operand union at 16/32/36/40 |
| Semaphore | 40 | WordTarget/operation at 16/32 |
| Trap | 16 | None |
| Timestamp | 32 | Address/context at 16/24 |

Copy options bit zero selects source mode and bit one destination mode: Linear
zero, ScatterGather one. Other options bits are reserved. Each endpoint occupies
32 bytes: Linear stores base/offset at 0/8 and leaves the remainder zero;
ScatterGather stores table/count/index/offset at 0/8/16/24. Counts, indices,
offsets, lengths and addresses retain all 64 bits. No field is split across a
parent header and child record. All multibyte fields retain natural alignment.

Wire values are little-endian. Reserved fields, direct complete-case alignment
gaps and unused slot tails must be zero. Software using C stores initializes the
entire 96-byte slot and then fills the selected complete case, including the
shared header. Generated headers expose the common header and individual case
layouts. Event and management-ring formats are unchanged by this repacking.

## Completion, error events and notification

**HW-CE-2.** Successful commands do not write per-task responses or error events.
Software selects completion boundaries, for example Copy -> Fill -> Fence -> Trap.
Fence must not publish before prior operations in its defined scope reach their
required completion point. Trap alone is not evidence of that completion point.

**HOST-CE-2.** Software treats an interrupt as a notification, not a completion
record. It observes its Fence location and error/fatal state. Independent channels
must not advance one shared timeline without an explicit synchronization protocol.
Completion values must not be reused while old accesses or handlers can refer to
them. Error/reset handling must end failed or cancelled software waits without
waiting forever for a success value that will never be published.

**HW-CE-3.** Each channel retains a hardware-produced, software-consumed ring for
error events. A stopping command error latches the channel stop before attempting
event publication. The event identifies the failed task and carries its error
classification and required diagnostics. Later success Fence/Trap commands cannot
pass the failed command. No event-space check gates otherwise successful commands.

**HW-CE-4.** A full, corrupt or unwritable event ring is a reporting failure, not
backpressure. Hardware retains channel fatal status and requests its independent
notification without relying on that ring. There is no implicit publication retry
loop. Hardware must not overwrite an unread entry or advance producer on a failed
write. Permanent Fence/Timestamp publication errors use this same diagnostic/fatal exit;
repairable target PageFaults instead retain the command for owner resume.

**HOST-CE-3.** Consuming an event does not clear the channel stop, acknowledge reset,
prove retirement, or permit memory reuse. Recovery follows channel reset and the
existing firmware/owner authority. DMA error events are separate from the PF
management event ring.

**HW-CE-5.** Response-consumption-dependent IOC state is removed with the response
protocol. Trap, error-event and fatal notifications have explicit cause/mask/ACK
rules. Mask changes do not replay old notifications, and platform-pending messages
retain their original owner through reset or reassignment.

**HW-CE-13.** The execution stage that detects a stopping error invokes its owning
Channel's reporting operation. Task identity and reset generation are checked
before reporting, including before event-memory effects. Channel owns the stop,
event push, notification and reporting-failure fatal transition. A later stage
propagates the stop without another report; success does not visit an event pipeline.
Reporting does not retire the accepted task or release resident pipeline state.
The common retirement boundary performs terminal cleanup; suspended/waiting work
retains its accepted ownership. An error IRQ therefore cannot make a subsequent
reset finish before all affected resident work exits.

## Atomicity, ordering and cancellation

**PLAT-CE-2.** Atomic operations are indivisible against all participating accesses
to the same storage, including overlapping ordinary accesses. They either commit
once and return their observed value, or fail before modification. A DMA-local lock
that other accessors bypass is insufficient. Acquire/release visibility is a
separate requirement, not implied merely by using a Rust mutex in the model.

**HW-CE-6.** Successful atomic updates and SEM acquisitions are checkpointed before
any later access or notification. A later fault, pause or reset cannot repeat or
silently reverse that effect. Testing count > 0 followed by an unconditional
decrement is not a valid SEM acquisition with competing waiters.

**HW-CE-7.** Drain waits for ordinary dependencies. Abort/Reset can cancel unfinished
waiting work, but reset success still requires resident cleanup. Cancellation is
not rollback of completed writes or consumed permits. Existing payload PageFault
keeps its external MMU report and owner resume semantics, without a duplicate DMA
error event; successors remain blocked until actual completion or cancellation.

**PLAT-CE-3.** RTL selects unsupported, exclusive-access or native-atomic support.
Software can discover the capability; unavailable commands are rejected explicitly
before effects, never emulated with non-atomic read/write pairs. Exclusive and
native-atomic implementations preserve the same command semantics. Contention
retry is distinct from a bus error. Wait and Fence do not require atomic RMW support.

**MODEL-CE-1.** The model uses existing generic atomic memory primitives and bounded
steps. It does not add exclusive monitors, bus transaction timing or a specialized
PDMA semaphore API to generic runtime. Semantic waiting remains visible; hardware
leaf stages do not invoke predecessor stages or add a global execution controller.

## Word operations and visibility

**HW-CE-8.** Write, Wait, Atomic, SEM and Fence use a `WordTarget` containing an
address, context ID and 32/64-bit width. Addresses must be naturally aligned and
the addressed range must not overflow. Values are little-endian; only the selected
width's low bits participate. Invalid geometry is `InvalidOperation` before access.
Fill accepts byte-aligned ranges of any length and repeats the low 1, 4 or 8
little-endian pattern bytes, including a partial tail. Overflowing ranges fail
with `InvalidLength`; zero-length Fill performs no memory access.

**HW-CE-9.** Wait supports equality/inequality, unsigned and signed ordered
comparisons, and `Reached`. Signed comparisons interpret the selected width as
two's complement. `Reached` tests whether the width-truncated signed difference
`observed - reference` is nonnegative. Software must keep the relevant distance
strictly below half the counter range; otherwise wraparound ordering is ambiguous.
A successful Wait is an acquire boundary for later accesses in that channel.

**HW-CE-10.** Atomic supports wrapping Add/Subtract, And/Or/Xor, Exchange and
single-attempt CompareExchange with Relaxed/Acquire/Release/AcquireRelease ordering.
It has no software old-value destination. The generic memory primitive returns
old bits for internal CAS/SEM decisions, but no result writeback is performed.
CAS mismatch completes with an unchanged target, not an access error. Completion
does not report whether CAS matched; a later concurrent load is not its fetch result.

**HW-CE-11.** SEM uses an unsigned 32/64-bit count. Signal increases it by one with
release semantics; maximum-value overflow produces `ArithmeticOverflow` without
modification. Wait atomically consumes one iff positive, with acquire semantics.
An unsuccessful compare-exchange retries through bounded steps, releasing the
grant; an observed zero does not subtract. Competing waiters cannot consume the
same permit. These operations require the corresponding atomic capability.

**PLAT-CE-4.** Fence is a release boundary for all prior accesses in the same
channel, including accesses to other contexts. It publishes one naturally aligned
32/64-bit software-selected value after those accesses become visible in the
platform synchronization domain. It does not drain other channels. Integrations
must provide non-tearing observation of completion/dependency words and appropriate
cache/interconnect ordering. Ordinary reads/writes are sufficient if the platform
provides these guarantees; native RMW instructions are not required for Wait/Fence.
The synchronous model checks functional order, not cache propagation or AXI timing.

**HOST-CE-4.** Software retains target storage and context bindings until completion
or acknowledged cancellation. Copy payload, Write, Fill, Wait, Atomic, SEM, Fence
and Timestamp target PageFaults retain the command and require owner RESUME. A
failed access reported as repairable must not have committed effects; earlier
successful accesses do not replay. A parked Fence still blocks successors and
must preserve its release ordering when it finally publishes. Permanent write
failures become stopping `WriteError`. Command and SG metadata and event publication
retain their fatal/stopping policies; this change does not make them pageable.

## Timestamp and external time

**HW-CE-14.** Timestamp carries a destination address and context, with fixed
eight-byte alignment and a little-endian `u64` result. Invalid alignment or range
overflow is `InvalidOperation` before sampling or memory access. The timer is
sampled when the command reaches execution, not when submitted or fetched. Prior
same-channel commands must finish before that point. Timestamp is not a cross-channel
barrier or a substitute for Fence release semantics. Software can submit
`Timestamp(start) -> Copy -> Timestamp(end) -> Fence -> optional Trap` and observe
the Fence before reading the samples under the platform visibility contract.

**HW-CE-15.** One accepted command retains its first sample, including zero, until
retirement. A target PageFault suspends the channel without a DMA event/IRQ and
releases the shared grant. Resume retries only the write with that sample, even
after repeated faults. It does not recapture time. Reset/Abort cancels retained
samples under the same rules as other target writes, without publishing a success.
Context Drain observes parked Timestamp/Fence references and reports ChannelStopped;
it cannot authorize early unbinding. Other channels remain eligible.

**PLAT-CE-5.** The assembler connects a read-only `dma::Timer` input via
`Dma::with_timer`. This is a direct sample, not an MMIO/register protocol; no timer
registers or CPU time calls are added to DMA. Without a source, Timestamp reports
stopping `Unsupported`. The platform owns units, frequency, epoch and source
lifetime. Sampling is side-effect-free. Channel and DMA reset do not reset the
external source. Counter rollover uses modulo 2^64; software measuring intervals
must keep an unambiguous elapsed range and must not assume synchronization between
different platform clocks. Frequency discovery/conversion is platform-owned, not
a new PDMA command or register interface.

**MODEL-CE-2.** The managed scenario harness owns an external wrapping tick counter
advanced once per top-level hardware interaction, including idle steps. DMA merely
samples it. Tests can inject controlled values and advance time independently.
No wall-clock dependence, background timer task, cycle-accurate timing or inference
of real RTL latency is implied by these model units.

### Decision and evidence, 2026-10-23

ROCr emits GET_GLOBAL_TIMESTAMP before and after copy work, then separately publishes
completion and notification. Its SharedSignal owns 64-bit timestamp storage;
GPU clock conversion belongs to the runtime/platform. These support the separation
of sampling, storage and completion, not a claim that PDMA reproduces an AMD clock
or every generation's ordering rules. Sources: [SDMA command submission](https://github.com/ROCm/ROCR-Runtime/blob/master/src/core/runtime/amd_blit_sdma.cpp),
[timestamp storage](https://github.com/ROCm/ROCR-Runtime/blob/master/src/core/inc/signal.h),
[clock conversion](https://github.com/ROCm/ROCR-Runtime/blob/master/src/core/runtime/amd_gpu_agent.cpp).

No sufficient opcode-specific evidence was found for how SDMA retries a failed
Timestamp write or retains its sampled value. PDMA therefore specifies its own
explicit retained-sample behavior. The former non-faulting Fence requirement was
an unnecessary PDMA special case, not a demonstrated SDMA constraint: page repair
uses the independent platform fault path, so it need not depend on successful
Fence publication. Uniform target-fault recovery preserves both ordering and the
existing software lifetime obligations. External timer simulation may use ticks;
the earlier warning against treating step counts as physical time still applies.

## Error records, reporting and discovery

`Event` is a 24-byte, 8-byte-aligned error record: ID at 0, code at 8, error info
at 16. Codes 1..8 are InvalidLength, ReadError, WriteError, LengthMismatch,
ListReadError, Unsupported, InvalidOperation and ArithmeticOverflow. Code zero
is invalid: no success event exists. Error info contains `bytes_completed`, the
committed Copy/Fill prefix; word operations report zero on pre-effect failure.
Atomic mismatch is not an event. Command decode failure is CommandRead fatal.

**HW-CE-12.** `irq_cause` exposes current event non-empty, sticky Trap, sticky
command-error stop and channel fatal. The two routine masks cover event non-empty
edge and Trap. Error/fatal bypass those masks; `irq_enable` gates all channel
notifications. Each executed unmasked Trap may emit even if its sticky bit is
already set. `trap_ack.request=1` clears that bit, not platform-pending delivery;
zero has no effect. Consuming an event does not ACK Trap. Unmask/enable never
replays either cause. Software must inspect causes/completion storage before waiting.

Fatal codes distinguish CommandRead, EventWrite, EventFull and EventConfiguration.
They do not rely on event memory being writable. Read-only discovery advertises
atomic32/atomic64 independently. `DmaConfig` sets capabilities at instantiation;
ordinary constructors default to neither capability. Platform assembly must not
advertise an implementation it cannot supply across the enabled address spaces.
An advertised operation rejected by a backend still reports Unsupported safely.

## Atomic result decision and evidence

On 2026-10-22 the user approved omission of software old-value writeback. This
keeps a single memory effect and avoids a second destination lifetime/fault/replay
contract. It does not reduce the generic atomic primitive's returned-value API.
AMD's [SDMA packet definition](https://github.com/GPUOpen-Drivers/pal/blob/c5e800072a32f68b6ccc4422936d96167c6e0728/src/core/hw/gfxip/sdma/gfx12/gfx12_merged_sdma_packets.h#L80-L160)
has target and operand fields but no independent result address; ROCr's
[atomic decrement builder](https://github.com/ROCm/ROCR-Runtime/blob/637eaec61186cc1f73480d99095ac4a3b2597242/src/core/runtime/amd_blit_sdma.cpp#L935-L949)
uses the target update for completion. This supports the selected interface,
not a claim that every SDMA generation hides every returned value.

## Verification status

Executable coverage added on 2026-10-22 and extended on 2026-10-23
(paths below are relative to `crates`):

| Contract | Executable evidence |
| --- | --- |
| Typed commands, invalid selectors, errors-only events and C interoperability | `contract/tests/{copy_engine,scalar_selectors,event_info}.rs`, `contract/tests/header_c/copy_engine.rs` |
| Write/Fill/Fence, all Wait comparisons, every Atomic kind and both widths | `dma/tests/command_operations/{flow,comparisons,atomic_widths}.rs` |
| Unsupported capability/backend, competing SEM waiters and overflow | `dma/tests/command_operations/atomic.rs`, `dma/tests/command_operations/faults/recovery.rs` |
| Retained scalar faults, committed-prefix protection and reset before/after effects | `dma/tests/command_operations/faults.rs` and its `faults/recovery.rs` |
| Wait peer progress, Drain/Abort and reset cleanup | `dma/tests/command_operations/{flow,maintenance}.rs` |
| Error full/configuration/write failures and no success-event admission gate | `dma/src/stages/tests.rs`, `dma/tests/{copy_event_protocol,event_ownership,irq_publication}.rs` |
| Trap mask/ACK, pending delivery and notification ownership | `dma/tests/{interrupts,interrupt_identity,mmio}.rs` |
| Authorized Function/PASID atomics and source tags | `hw-components/tests/{windows/memory,platform_tags/authorization}.rs`, `scenarios/tests/firmware_managed/harness/runtime/tests/atomic_memory.rs` |
| Actual direct/managed software completion and reset while waiting | `scenarios/tests/firmware_managed/harness/runtime/tests/copy_engine.rs`, direct single/multi-channel scenarios and Host cancellation tests |
| Timestamp wire/C encoding, repeated write faults and retained zero sample | `contract/tests/timestamp.rs`, `contract/tests/header_c/copy_engine.rs`, `dma/tests/command_operations/faults/timestamp.rs` |
| Timestamp peer progress, context maintenance and reset cancellation | `dma/tests/command_operations/faults/timestamp/lifecycle.rs` |
| Platform timer and Host-observed Timestamp/Copy/Fence across managed reset | `scenarios/tests/firmware_managed/harness/runtime/tests/copy_engine/timestamp.rs` |

These checks exercise functional memory, register, queue, interrupt and lifecycle
behavior. They do not certify physical exclusive-monitor protocols, coherence,
posted-write visibility, outstanding transaction drain or bus timing. Those remain
explicit RTL/platform obligations under PLAT-CE-2/3/4 and the reset chapter.
