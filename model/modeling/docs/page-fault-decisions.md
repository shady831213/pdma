# Pageable-copy scheme: decision history and evidence

Recorded 2026-10-09. This records the complete scheme adjustment, including the
subsequent removal of suspension tickets. The numbered requirements in
[page-fault-recovery.md](page-fault-recovery.md) are normative. Earlier alternatives
below are history, not additional interfaces to implement.

## 1. Start with the recovery boundary, not the vendor name

The discussion separated two uses of the same copy engine. A migration service
copies prepared page contents after software has made its source, destination,
queues and SG metadata accessible. That service needs the
[non-faulting migration preconditions](context-contract.md#non-faulting-page-migration-copy),
not a pager inside DMA. In contrast, an ordinary copy between pageable virtual
addresses may itself encounter a repairable missing mapping. Its original request
must remain meaningful across repair, while unrelated work can continue.

There are also two possible platform access boundaries. The existing external
ATS+PRI port transparently holds a transaction through repair and retries it
internally. PDMA sees a stall at that boundary, not an explicit PageFault outcome.
The approved integration here instead allows a GPU-style platform backend to
return a classified payload fault and separately report its identity, address and
access. PDMA then parks a channel and requires owner resume. These are different
contracts; layering both recovery procedures on the same retained transaction
would create competing owners. No true MMU or integration with the external port
is implemented or validated by this change.

Linux describes PASID address-space identification, device translation caches,
MMU-notifier coherence and page requests; these establish why translation repair
requires platform services beyond storing a PTE. They do not define our physical
channel interface. [Linux SVA documentation](https://docs.kernel.org/arch/x86/sva.html)
and the [Linux page-request structure](https://github.com/torvalds/linux/blob/v6.12/include/linux/iommu.h#L42-L74).

## 2. The earlier terminal partial-completion alternative

The 2026-09-28/29 proposal made a payload fault a terminal response. Hardware
reported the committed byte count and source/destination continuation positions;
software repaired the mapping and submitted a new residual request. That proposal
used a 72-byte response, status 6 and fault information, retained no suspended
command, and could discard read scratch before a failed destination write.

The analogy had a specific source: Intel's DSA guide describes partial completion
when Block On Fault is clear, including fault address, processed bytes and software
descriptor adjustment/resubmission. It also describes blocking as another policy.
Thus "DSA-like" never meant that all Intel operations terminate on every fault.
[Intel DSA User Guide, section 4.1.5](https://cdrdv2-public.intel.com/759709/353216-004-intel-data-streaming-accelerator-user-guide.pdf#page=26).

Partial completion was a coherent alternative, but it assigned continuation to
software and completed the accepted task at each fault. The selected requirement
instead keeps that task and its queue position alive through repair. Consequently
the fault response, residual-command resubmission API and scratch-discard rule are
retired together. A 24-byte normal response now completes the original command;
statuses 0..5 remain valid and former PageFault status 6 is rejected.

The motivating dependency example was `Copy A -> Signal A -> Copy B`. Signal/fence
commands are future discussion, not a currently implemented opcode or an observed
bug in today's model. A terminal PageFault response for A would say that this
hardware descriptor ended; it would not say the logical copy had finished.
Resubmitting A's residual at the ring tail cannot automatically preserve A's
original place before Signal A and B. Treating partial descriptor completion as
logical completion could therefore release a future dependency too early.

One possible contract would give software responsibility for a whole batch's
recovery, withholding or reconstructing its dependent signals/fences and later
work until all residual copies complete. That requires an explicit batch and
dependency protocol. The selected contract instead retains A in its physical
channel and blocks that channel's successors until A completes or is cancelled.
This preserves its queue position directly, while other physical channels can
run. It motivates retention without silently implementing future fence semantics.

Independent endpoint starts remain useful: Linear byte offsets or SG entry plus
entry-local offset, and one common copy length. They describe ordinary selected
ranges even without a fault. Retaining them avoids rebuilding residual SG lists
or searching a global byte-offset prefix, but no longer makes them a continuation
response protocol. The etha_model RTL implementation informed the discussion of
bounded comparisons, subtraction and address adjustment. This is a structural
observation from that discussion, not a measured area/timing result or public
proof of this recovery mechanism.

## 3. Why retain the physical-channel operation

The approved task lifecycle is accept, execute, optionally park and resume, then
complete or cancel. A payload fault releases the shared engine but retains one
bounded operation on its physical channel. Same-channel successors wait; eligible
peers use the existing arbiter. One fault does not require another engine, another
hardware channel, a logical-channel namespace or a firmware scheduler.

Retained state includes the original command ID, independent cursors, committed
prefix, copy phase and successful read scratch. Only a successful destination
write commits progress. If that write faults, retry its retained bytes rather
than rereading source memory that might have changed or overlapped earlier writes.
The platform must guarantee that a PageFault write committed no bytes from that
failed call. This does not add a memmove guarantee or roll back prior writes.

Retaining only a transfer snapshot is insufficient for the composable model:
restarting the command graph can repeat range checking, completed transforms or
non-idempotent custom stages. The command arbiter therefore parks the exact composed
continuation and restores it when that channel receives another grant.
`RunCommand` ownership remains live until full terminal retirement, including
post-publication stages. Per-channel accepted references survive parking so
context cleanup cannot mistake a yielded engine for a completed command.

The 2026-10-10 structural review removed the initial outer `Schedule` wrapper.
That design arbitrated only Fetch inputs and later signaled `resume_selected` to
a second layer which swapped whole operations. New-command selection and parked
restoration consequently had separate owners. The replacement arbitrates a whole
channel execution turn: a channel-bound port either fetches after its grant or
restores the existing operation, retaining the grant until completion or payload
fault. `CommandArbiter` supplies one shared engine and per-channel operation slots
to the existing `Repeat<Arbitrate<...>>` composition. No second scheduler, generic
pipeline extension, per-channel engine or scratch reconstruction is required.
Reset/Abort cleanup runs before arbitration so it does not depend on eligibility
or progress of an unrelated disabled channel. This changes assembly ownership,
not the command-atomic scheduling or fault/lifetime contract.

The 2026-10-12 value-flow refactor supersedes that execution-port structure.
Each channel now produces a non-consuming candidate value; DMA's round-robin policy
selects among completed values, and one shared output engine accepts the selection.
Input advancement, planning and output advancement are separate Fan phases.
The engine retains a healthy invocation, or saves its exact child continuation on
fault and completes the current turn. The saved-state bank is data, not a bank of
engines, and the temporary context never contains an executable stage. Cleanup still
runs independently of candidate eligibility. This removes ReadyStage and the old
coupling between arbitration policy and driving selected source hardware.

Hardware steps now report only InProgress or Complete. The async executor yields
after every bounded step, allowing modeled software to run between steps without
hardware wait tokens. This deliberately trades idle polling efficiency for a direct
step-based hardware model. Software interrupt and doorbell waits remain unchanged;
this does not claim cycle timing or real bus retry behavior.

Validation exposed a scheduler distinction: an immediately self-waking future can
return Pending yet repeatedly run ahead of software deferred by Tokio's yield.
Three in-flight reset scenarios observed the entire 1024-byte copy before software
regained control. The executor therefore requests one backend-provided scheduler
yield after each committed step; Tokio adapters use Tokio's yield primitive.
This keeps scheduler mechanics outside stage logic and avoids a framework dependency
on a particular async runtime. Completion remains retained across cancellation
of that yield, and neither queue progress nor software notifications become wait tokens.

The real RTL obligation is equivalent observable commit and lifetime behavior,
not Rust object storage or one synchronous memory transaction at a time. Ordered
command completion does not imply serial physical transactions. A more parallel
implementation needs its own accounting for completed writes, fault boundaries,
retained data, cancellation and retirement. None is inferred from this model's
deterministic synchronous steps.

In particular, ordered completion is an externally visible ordering property;
it does not prove that a later command has issued no reads or writes when an
earlier command faults. The patent's ordered copy stream and pipelined reads
provide a concrete reason to separate those questions. RTL/integration must
account for outstanding and later-issued work when choosing commit, replay and
cancel boundaries. It cannot simply ignore those transactions or infer their
absence from in-order responses. The current synchronous model has no such bus
transactions, so its tests cannot discharge that integration obligation.

## 4. What the GPU evidence does and does not establish

Pinned NVIDIA UVM source describes CE/PBDMA fault-and-switch, TSG blocking, repair
and explicit clearing before rescheduling. Its clear path uses MEMOPS or direct
register access; service trackers order repair before clearing. This supports the
choice to resume retained work rather than require a residual descriptor from the
application. It does not reveal complete checkpoint storage or firmware/RTL
ownership. [NVIDIA UVM source](https://github.com/NVIDIA/open-gpu-kernel-modules/blob/61dcc93722ecb418bb5f2e00923f05b4b8051dd1/kernel-open/nvidia-uvm/uvm_gpu_non_replayable_faults.c#L38-L112).
The inspected implementation is
[clear_faulted_method_on_gpu/register_on_gpu](https://github.com/NVIDIA/open-gpu-kernel-modules/blob/61dcc93722ecb418bb5f2e00923f05b4b8051dd1/kernel-open/nvidia-uvm/uvm_gpu_non_replayable_faults.c#L286-L366)
and the [successful-repair-to-clear call](https://github.com/NVIDIA/open-gpu-kernel-modules/blob/61dcc93722ecb418bb5f2e00923f05b4b8051dd1/kernel-open/nvidia-uvm/uvm_gpu_non_replayable_faults.c#L760-L790),
not an inference from the introductory comments alone.

The NVIDIA copy-engine patent describes preserving copy methods and replaying the
faulting copy operation while allowing work switching. Its granularity differs
from our exact retained scratch and composed-operation rule; it is evidence of an
alternative implementation strategy, not proof that a particular shipped GPU
uses it. [US20180314431A1](https://patents.google.com/patent/US20180314431A1/en).

Public [Turing PBDMA registers](https://github.com/NVIDIA/open-gpu-doc/blob/9fdf5c4062007929d9f4e6cbad9c9771fe61b880/manuals/turing/tu104/dev_pbdma.ref.txt)
and [pushbuffer encodings](https://github.com/NVIDIA/open-gpu-doc/blob/9fdf5c4062007929d9f4e6cbad9c9771fe61b880/manuals/turing/tu104/dev_ram.ref.txt)
describe dispatch formats. [NVIDIA's RISC-V presentation](https://static.sched.com/hosted_files/riscvsummit2024/fe/Keynote_2024.pdf#page=11)
shows GSP/RM and guest-associated partitions. Neither establishes a single VF FIFO
combining ordinary copies, reset and resume, nor the complete guest-to-CE transport.
TSG grouping alone does not prove oversubscription.

The precise split between fixed logic, firmware and software for GPU checkpoint
save/restore remains unresolved. "Pure RTL preserves everything" and "firmware
reconstructs and resubmits every residual task" are both unsupported conclusions.
AMD retry behavior was discussed without enough checked evidence for a universal
explicit-clear or automatic-retry claim. No NVIDIA or AMD hardware execution was
tested. Vendor names cannot substitute for the chosen integration contract.

## 5. Simplify resume: reject the ticket extension

An intermediate implementation added a monotonically allocated nonzero 64-bit
SUSPENDED ticket and required an equal RESUME value. It attempted to make arbitrary
late writes inert across later faults and reset/regrant. That required lifetime
counters, non-reuse rules across soft reset, overflow handling and an exhaustion
fatal/admission policy, plus special assumptions for hard instance recreation.

The motivating race was overstated. Suppose fault A stops the channel, request A
is applied, and then execution reaches fault B. A later request that incorrectly
clears B must be a second request, a duplicate or a separate old actor's request:
latency of the one request already consumed to reach B cannot create another write.
The supported owner recovery flow does not require competing resume writers or
blind duplicate retries. Tickets therefore solved a stronger protocol than the
agreed requirement and imposed unrelated failure modes.

Tickets also cannot by themselves correlate a stale platform report. An old
handler that reads the current ticket after B can obtain B's value and submit a
matching write. Mapping repair can likewise target recycled OS state even if a
DMA write is rejected. Software still has to associate reports with live work and
serialize recovery with lifecycle changes. Those obligations cannot be replaced
by reading a hardware number.

The final interface is four-byte read-only `FaultStatus.faulted` and four-byte
write-only `Resume.request`, each at bit 0. A write-one request clears the current
faulted bit. It does not enable, clear errors or complete work. Zero and a request
while not faulted do not arm a future resume. A repeated fault sets the bit again.
`faulted()` returns the boolean through the ordinary fallible software register
API, and `resume()` takes no ticket. There is no counter or exhaustion condition.

The tradeoff is explicit: a duplicate or late request arriving during a new fault
can clear that fault. Owner software must serialize fault handling with context
maintenance, reset, release and reallocation; retire old handlers, mapping edits
and pending writes before reuse; and prevent an already serviced report from
issuing another request. Ordinary ordering for one MMIO issuer does not prove
coordination between CPUs, PF/VF actors, transports or lifecycle operations.
Platform-specific synchronization and completion are still required where those
actors exist. This decision promises no universal stale-write protection.

## 6. Fault, error and fatal remain different outcomes

Recoverable payload PageFault is a suspended accepted task, with no response,
DMA interrupt, response-error cause or firmware event. A normal stopping payload
error completes with the committed prefix and stops the channel; fatal metadata
or publication failures retain their existing scoped policy. SG metadata faults
remain stopping list errors. An inaccessible model region, permission rejection
or allocation failure is not automatically repairable paging. Further repairable
classes, such as copy-on-write, need a separate classification agreement.

The platform reports address space, fault address and access independently. Owner
software authorizes repair and completes required translation visibility before
resume. Firmware continues to own grants and context/channel lifecycle, without
a pager, new resume opcode, fault event or required PF proxy for VF work MMIO.

Drain encountering parked work returns ChannelStopped and the retained failure
bitmap so the serial firmware control queue can continue. Abort discards matching
parked work, including a running target that parks after Abort acceptance. Reset
also cancels parked work without resume. Cleanup must retire accepted references
before success; committed writes remain. Failure, timeout or dropping a waiter
never grants permission to free buffers or rebind contexts.

## 7. Future scope and verification boundary

Future oversubscription would require logical channels exposed to software and a
managed mapping onto physical execution resources. It must not expose physical
registers as if logical channels were permanent hardware slots. Firmware policy
alone is insufficient: hardware must provide suitable saved-state, quiesce,
retirement, switching and isolation primitives and the interfaces need defined
ownership/lifetimes. The current fixed physical channels are not that feature.

Barrier waits, visibility, signaling, completion ordering and scope remain a
separate discussion. This change adds no implicit fence, barrier opcode, PCIe
integration, FLR or true MMU. The external transparent-retry boundary remains a
separate possible integration.

The [verification obligations](page-fault-recovery.md#verification-obligations)
require wire/access checks, read/write and repeated faults, retained write scratch,
non-idempotent composition, peer progress, reset/Abort/Drain and managed ownership.
Model tests establish those modeled outcomes only. Public-source review establishes
the bounded comparisons above. Platform fault correlation, OS repair/invalidation,
cross-actor handler retirement and physical outstanding-transaction cleanup still
need integration/RTL evidence; this record does not report them as implemented.

## 8. Preserve fault origin with a platform-defined tag

Review of the explicit-resume path exposed an attribution gap. DMA's DmaAccess
already named its channel and context, but the context router resolved it to only
Function/PASID MemoryIdentity. The original injected backend retained that identity
and reported address/access without a source. A one-channel scenario could select
its known driver and pass, but two channels sharing the same address space could
not be distinguished by that report. Address-space isolation did not establish
fault-source attribution.

Two designs were considered. Making every MMU understand a PDMA ChannelId would
carry the required information, but bind a platform component to one engine's
namespace. The chosen design instead lets the platform construct an opaque tag
after hardware owner/context authorization. The MMU echoes it; the platform's
software-facing association resolves it back to the correct source. Other engines,
multiple DMA instances and different bus tag encodings do not require a new PDMA
descriptor or a modified generic memory framework. MemoryIdentity continues to
select the address space. The tag identifies origin, not permission or PASID.

The 2026-10-10 review names this source association `ChannelTag` and its request
and report field `ch_tag`, replacing the ambiguous `FaultTag`/`tag` names. It is
not merged into MemoryIdentity or context attributes: several channels can share
an address space, and one channel can access several contexts. The platform
continues to define its representation; the name does not mandate a channel-ID
encoding or introduce a new DMA register/descriptor field.

Direct encoding and association tables are both valid platform choices. A table
is not required merely because the value is called a tag. Whatever representation
is chosen, its association must outlive the access, queued report and handling;
reuse for another source before these finish is invalid. Stable source tags do not
solve old-handler races across channel regrant, so the previously agreed software
lifecycle exclusion still applies. This is distinct from the rejected per-fault
DMA suspension ticket: no fault counter, matching resume token or overflow fatal
returns to the DMA model.

The evidence for this change is the inspected dataflow and same-space multi-channel
behavioral tests, not a new inference about GPU internals. See PLAT-PF-02 and the
source-correlation verification obligations in the normative chapter. Real bus-tag
capacity, transaction completion and external MMU report transport remain platform
integration responsibilities.

The scenario platform uses an immutable per-instance source catalog with opaque
reference tokens. Requests and reports retain their tokens, so another catalog
cannot accidentally acquire the same live association. The injected MMU sees a
tag and address-space identity, not a PDMA channel index. Software creates a
handler scope for granted channels and retires that scope before reset/release.
This is a modeling choice, not an RTL pointer encoding or a required lookup table.
A new handler for the same physical source cannot infer that a report belongs to
an earlier grant: old-report draining and handler exclusion remain software duties.

The managed scenario deliberately submits channel 1 before channel 0 and checks
each report against the source that actually issued it. Both fault on the same
Function/PASID/VA before repair. Repairing the shared page and resuming one tagged
source leaves its peer parked until its own resume. Real reset and release/regrant
then exercise new work while the retired old handler rejects its retained report.
Separate routing tests check arbitrary nonnumeric tag types and that rejected
authorization invokes neither the platform mapper nor the memory backend.
