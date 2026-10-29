# Firmware-managed DMA roadmap

Recorded: 2026-09-15.
Updated: 2026-10-23.
Status: the architecture MVP, Function-scoped channel management, context selection
and maintenance, and managed fatal/event recovery are implemented. Actual PCIe/IOMMU
integration and Function-level reset remain unimplemented. Payload-only retained
suspension with owner per-channel resume replaces the earlier DSA-like continuation;
the first Signal/Wait implementation has been withdrawn. The agreed next feature set
adds copy-engine commands and replaces DMA success responses with ordered completion
writeback, optional notification and a per-channel error event ring. These additions
are implemented and model-verified; [copy-engine.md](copy-engine.md) specifies
exact command, error-event and visibility semantics and records verification.
HSA remains a semantic reference, not a mandatory AQL migration or conformance goal.
Generic memory atomics and reset during context maintenance remain implemented.
This document records decisions and open questions; it is not an execution plan
or an instruction to start implementation automatically.

The next phase is RTL architecture preparation against the implemented system
contract. [RTL design inputs](../../../docs/rtl/architecture.md) records the agreed AXI5 master,
32/64-bit AXI4-Lite slave, three clock domains, configurable geometry and tentative
22nm/500MHz target. Tool/EDA flows are assumed available for this phase; it does
not start a toolchain split or add model features. Register commit and transaction
retirement details still require closure before RTL coding.

On 2026-10-29 the first RTL phase was scoped to DMA only. CPU, managed peripherals
and subsystem integration follow DMA convergence. Existing DMA-maintenance behavior
is retained, but peripheral ContextRequest register redesign is not a prerequisite.
The same day's overlap clarification restates software ownership: no hardware
overlap detection/rejection or portable within-Copy overlap result is promised;
see [HOST-CE-5](copy-engine.md#software-owned-overlap-policy).

First milestone: the [firmware-managed DMA architecture MVP](firmware-dma-mvp.md).
The subsequent [control channel](control-channel.md) implements independent
command/response rings for multi-channel acquisition/reset/release, including local
reset completion. [Function-aware ownership](function-isolation.md) now covers MMIO,
management and notification routing. Function-selected memory capabilities now
provide isolated model address spaces. Default/PASID context attributes and routing
are implemented; actual address translation and external transaction drain are not.
See the [DMA memory boundary](function-isolation.md#dma-memory-boundary).
Validate the ownership and runtime assembly boundaries before implementing the
broader virtualization and address-space features below.

The 2026-09-20 [VF pre-FLR decision](function-isolation.md#agreed-vf-pre-flr-integration-contract)
uses existing PF-driven channel release before PCIe FLR. It supersedes firmware
handling of incoming VF FLR: no second firmware ingress or cleanup coordinator is
planned for this scope. Under the software precondition, application ACK can be
immediate; PCIe drain and controller handshake remain integration responsibilities.
No FLR implementation or new hardware/firmware interface is added by that decision.
PF reset and additional cleanup for future context/address-space resources remain
separate design work.

The 2026-09-21 [context contract](context-contract.md) records the agreed selectors,
control bootstrap configuration, context lifecycle and software obligations for
modeling and RTL. Context selection and lifecycle behavior are now implemented;
translation and bus transaction mechanics remain outside the model. Its numbered
software/hardware requirements supersede the previously open selector and
drain/abort questions here and must be retained in the eventual RTL specification.

## Next direction: GPU/SoC copy engine

Recorded: 2026-09-28. Status: use case accepted for design discussion, not an approved
interface or implementation plan. PDMA may be integrated into a GPU-like SoC as a
copy engine participating in HMM-related page migration and page-table maintenance,
in addition to the PCIe endpoint use case. Keep page-data movement distinct from
page-table updates and MMU invalidation. Ordinary copy must remain simple.

### First: page-fault and recovery semantics

Decision, 2026-09-28: separate two uses. For migration of prepared page contents,
software/platform guarantees a non-faulting copy path, including supporting queue
and SG accesses. Existing DMA copy behavior is sufficient for that data-movement
step; unexpected access failures remain errors, not recoverable paging. This is a
chosen usage contract, not a universal restriction on migration engines. The
[context contract](context-contract.md#non-faulting-page-migration-copy) records
the normative obligations and the boundary with full HMM support.

The second use copies pageable virtual addresses, where DMA's own payload access
can encounter a recoverable page fault. The 2026-10-09 decision selects retained
suspension and owner resume, superseding terminal partial-completion/resubmission.
DMA retains the original command, both cursors, committed prefix and successful
read scratch when a write faults. Same-channel successors wait; other physical
channels can use the shared engine. The platform MMU/backend independently reports
fault identity/address/access; software observes FAULT_STATUS.faulted, repairs
translations and writes RESUME.request=1 through its owned channel work MMIO. There is
no intermediate DMA response, IRQ, firmware event or new PF management opcode.

The [page-fault recovery chapter](page-fault-recovery.md) records the contract,
software lifetime requirements and superseded decisions. Independent Linear offsets
and SG entry/local-offset positions remain general subrange inputs. The copy-engine protocol replaces final success responses with explicit Fence/Trap
and 24-byte error events; no PageFault event or continuation fields are retained. Fault status and resume are 32-bit registers with one boolean bit each;
software must serialize repair/resume with reset/release/reallocation and retire
old fault handling before reuse. Reset/release cancel parked work without resume. Drain reports ChannelStopped when
parked work prevents completion; Abort discards matching parked work without a
response. Firmware retains resource lifecycle management. The ABI is not frozen.

Firmware-owned logical VF queues remain future work. TSG-style grouping does not
imply channel oversubscription, and a future logical channel interface should hide
physical registers. Firmware alone cannot supply oversubscription without hardware
state-save, lifecycle and isolation primitives. Physical channel count stays fixed in this slice. Transparent
MMU/ATS+PRI retry is a distinct lower boundary: a port that retains and retries the
original transaction does not return a PDMA PageFault or need PDMA resume. No
integration with that port or true MMU is included.
Keep the existing functional abstraction: no bus cycles, transaction IDs,
outstanding queues or asynchronous memory-transaction model are requested.

### Copy-engine synchronization reassessment

Decision, 2026-10-21. This section supersedes the 2026-10-19 staged plan for
the complete HSA signal API followed by mandatory AQL convergence. The user
approved withdrawing the Signal/Wait implementation before choosing its replacement.
Use SDMA as a copy-engine reference and HSA as a system-semantics cross-check;
neither complete SDMA instruction coverage nor HSA/AQL compatibility is a current goal.

The Copy-only implementation is restored to
`a67dbfce2e867b48a571fcb4b39874057d9f1932`. The withdrawn implementation and
specification remain in `247e914` and `50b38bc`. Generic memory atomic32/atomic64,
backend-owned errors and reset during pending context maintenance are retained.
The Signal/Wait-specific descriptor, response variants, software helper, routed
atomic extensions and their dedicated tests are withdrawn together. The withdrawal
restored managed memory adapters to their read/write-only baseline. Preparation on
2026-10-22 adds authorized atomic forwarding through those data adapters and platform
memory selection; it does not yet add DMA Atomic/SEM commands or change PF
control-memory capabilities.

The pipeline/command-continuation refactor remains complete at `ae3b7f3`.
Preserve its hardware ownership, typed dataflow and command/chunk completion
boundaries. Synchronization is not a reason to reopen the pipeline framework.
The earlier performance comparison did not establish a significant inherent
penalty from binary step outcomes; retaining command execution across chunks
removed repeated outer work. No further old-Pending comparison is requested.

#### Evidence scope and corrections

Sources were read on 2026-10-21. Concrete code examples below use ROCr
`rocm-6.3.3`, commit `637eaec61186cc1f73480d99095ac4a3b2597242`, and Linux
`v6.12`; the architectural queue overview is Linux 6.16 documentation.
They establish those software paths, not every SDMA generation or undocumented
hardware execution details. Newer APIs can add different packet capabilities.
In particular, the current ROCr API documentation describes per-operation raw
GPU-side wait/signal attributes separately from the HSA completion signal. That
is a further API design example, not evidence that every device supports the new
encoding or that the older builder covers all current submission paths. [S9]

- AMD's compute AQL queues and native SDMA queues are different submission paths.
  AQL packets carry completion-signal handles; the inspected SDMA copy builder
  does not consume AQL or embed that handle in its linear COPY packet. [S1, S2]
  This does not mean SDMA has no AQL support: public SDMA packet definitions also
  contain AQL layouts. Their existence does not establish support on every product
  or change which path the inspected builder uses. [S13]
- HSA completion is an atomic decrement after the applicable release fence.
  An HSA software signal API operation is not automatically a DMA opcode. [S1]
- A native FENCE packet, a Linux `dma_fence` software object, an HSA signal and
  acquire/release memory ordering are different concepts. Do not substitute one
  name or contract for another. [S1-S4]
- ROCr exposes SDMA engines through GPU-agent copy APIs. This does not establish
  that SDMA is a separately enumerated HSA "other agent", or that its queues
  implement the AQL agent-dispatch contract. [S5]

The prior discussion incorrectly used the SDMA fallback as evidence for AQL
packet-processor completion on a non-atomic platform. That inference is withdrawn.
We have not established the latter implementation.

#### Commands and software/hardware division

The inspected ROCr path composes native packets:

```text
dependency POLLs -> platform cache operations -> COPY packet(s)
                 -> platform writeback -> completion update -> optional TRAP
```

Its builder emits linear/rectangular copy and fill, equality polling, 64-bit
atomic decrement implemented as ADD64(-1), ordered immediate-value FENCE writes,
and optional notification. Larger copies are split in software. Submission
publishes the queue write pointer before ringing the doorbell. These are
observed builder choices, not an exhaustive SDMA ISA specification. [S2]

| Responsibility | PDMA owner to preserve |
| --- | --- |
| Select a dependency/completion protocol and allocate its storage | Host/shared software |
| Execute Copy, and any future wait/writeback/atomic operation | DMA hardware stages and channel-owned state |
| Bind contexts and authorize channel access | Firmware policy plus hw-components enforcement |
| Provide atomic capability, coherence, ordering and fault transport | Platform memory/transport implementation |
| Model clocks, CPU/ISR delivery and wire actors together | Scenario harness/runtime, without DMA policy |

The reviewed SDMA APIs do not justify moving PDMA execution policy into runtime,
using firmware to submit every VF task, or adding another pipeline controller.

#### Two concrete completion protocols

**ROCr copy API:** completion updates use an atomic packet when platform atomics
are supported. Otherwise runtime calculates the old value minus one at submission
and emits one or two FENCE writes. Blocking copies reserve private signals before
reuse. This verifies a fallback exists, not a general concurrent decrement
guarantee for the public asynchronous API. [S2, S5]

**Linux amdgpu ring fences:** software allocates a sequence number for a ring;
SDMA writes the supplied value and can emit a TRAP. The fence handler reads the
completed sequence and signals intervening software fences. This is a serialized
per-ring completion protocol, not a multi-producer shared countdown. [S3, S4]

The agreed PDMA Fence command supports a software-selected completion location
and value; a per-channel single-writer timeline is one usage. Different channels must not blindly
write one shared monotonically increasing location: a later value from one channel
does not prove another channel's work completed. Shared aggregation needs a
separate correct protocol, potentially atomics.

The non-atomic fallback must not be copied as a generic RMW emulation. For example,
if two submissions both read 2 before either completes, both precompute 1; ordered
writes of 1 then 1 do not produce the required countdown result 0. This is a
counterexample to unrestricted equivalence, not a reproduction of an AMD product bug.
The permitted sharing/lifetime conditions of that fallback remain unresolved.
Two 32-bit writes also do not independently prove an indivisible 64-bit publication.

#### Dependency waiting, ordering and progress

The ROCr builder polls both halves of a signal for zero, uses equality comparison,
and programs an indefinite retry setting. This is not proof of an atomic 64-bit
snapshot for arbitrary concurrent signal changes, nor proof that a stalled POLL
releases a particular SDMA pipe. [S2]

Linux documents queue-to-pipe mapping and switching; a pipe processes one mapped
queue at a time. That architectural overview does not specify precise scheduling
on every unsuccessful POLL or XNACK. [S6]

PDMA should keep the already agreed requirements when waiting is reintroduced:
an ordinary false dependency is not a PageFault, same-channel successors cannot
pass it, and peers needed to satisfy it must be able to run. Drain waits for normal
completion; Abort/Reset remain reachable cancellation paths. Those are PDMA
requirements, not claims of identical AMD scheduling.

The model's synchronous memory backend does not prove cache or interconnect
visibility. Required publication/acquisition points must be explicit before
adding synchronization. Cache management and TLB operations remain platform
responsibilities. SDMA's separate cache-management and VM-flush paths are evidence
against treating an ordinary completion write as an implicit universal cache/TLB
flush. [S2, S3]

#### Fault, preemption and reset

The reviewed GFX9 SDMA queue-destroy code disables ring admission, waits for a
context-idle observation, reports timeout failure and captures the read pointer.
The SDMA v7 IB-preemption path requests preemption and waits for a trailing fence.
These are concrete request/acknowledgment mechanisms; neither alone proves that all
external transactions have drained under every reset mode. [S3, S7]

PDMA already distinguishes reset request, resident cleanup and completion in
[reset.md](reset.md). It retains payload-fault cursors and read scratch, releases
the grant, and requires owner repair/resume, as specified in
[page-fault-recovery.md](page-fault-recovery.md). Keep these observable guarantees.

**Unresolved in the sources reviewed:** the exact SDMA payload replay checkpoint,
whether later packets have issued effects when one access faults, the notification
coalescing rule, and the precise retry/wakeup schedule for each generation.
Do not replace PDMA's explicit resume or replay-safety contract with inferred AMD
microarchitecture. Logical-queue oversubscription and true MMU/PCIe integration
remain deferred.

#### Gap matrix against the restored PDMA

This is the 2026-10-21 pre-implementation comparison, retained as decision
history. Its old file names and response-based behavior describe that checkpoint,
not current APIs. The following chapter's implementation status and
[copy-engine spec](copy-engine.md) supersede those gaps.

| Area | Restored PDMA evidence | Gap and proposed disposition |
| --- | --- | --- |
| Copy and SG traversal | `contract/src/dma/command.rs`; `dma/src/stages/execution/{validate,copy}.rs` | Retain Linear/independent SG. Add Write and Fill; rectangular/tiled and page-table-specific operations are outside this slice. |
| Dependency gating | `dma/src/stages/candidate.rs` only offers fetch/resume candidates | No Wait after withdrawal. Define comparison, width, context, lifetime and peer progress before implementation. |
| Ordered completion writeback | `dma/src/stages/responses.rs`; `dma/src/channel/response.rs` | Add Fence; replace the DMA response ring with an error event ring, not mandatory success records. Preserve diagnostics and an independent reporting-failure outlet. |
| Completion versus interrupt | Channel response publication and IOC/non-empty IRQ causes | Add independent Trap; remove response-consumption-dependent IOC bookkeeping and success-response backpressure. Specify event notification and fatal fallback separately. |
| Multi-writer atomic update | `hw-model-io/src/memory/atomic.rs` and storage tests | Add Atomic and counting SEM with authorized managed forwarding. Model indivisible operations; RTL selects unsupported, exclusive or native-atomic implementation. |
| Visibility and ordering | `docs/spec.md` RTL handoff and `docs/reset.md`; synchronous memory calls | Need a precise producer/consumer visibility contract and a model boundary to verify it. Do not equate Rust mutex ordering with platform coherence. |
| Queue/engine relationship | `dma/src/stages/data_path.rs`; retained work in Channel | One shared engine and fixed channels are intentional. Wait peer progress must be proven; oversubscription is not required. |
| Fault and reset | `dma/src/reset.rs`; `dma/src/maintenance/progress.rs`; fault/reset scenarios | Existing ownership and replay boundaries are usable. Define faulting synchronization accesses and committed-update checkpoints when new operations are chosen. |
| Descriptor compactness | 104-byte Copy command and 24-byte response; generated C ABI | The command families below are agreed; exact compact encodings and error-event layout remain to be specified, without mandatory AQL compatibility. |
| Software assembly | `software/src/dma.rs`, firmware resource management, managed scenario harness | Keep task compilation in software and execution in hardware. API similarity alone does not require importing HSA handles or runtime objects. |

All code paths in this table are relative to `model/modeling/crates` unless prefixed
`docs/`. These are design gaps and deliberate differences, not newly discovered
regressions in the restored Copy contract.

#### Agreed copy-engine feature set and implementation status

Decision, 2026-10-21, following the command and completion review. Preserve existing
Linear/independent-SG Copy and add the command families below. This records the
approved direction, not an instruction to implement automatically or restore the
withdrawn Signal/Wait ABI.

Implementation started on 2026-10-22 under explicit user approval. The
[copy-engine specification chapter](copy-engine.md) now fixes the first-slice
wire/operation details; its verification section records completed acceptance.
Timestamp was approved on 2026-10-23 with an external timer input and retained
sample on target PageFault; Fence now uses the same target-fault recovery as Write.

| Command | Agreed purpose and boundary |
| --- | --- |
| Write | Write command-carried data to an authorized target. Unify memory/register-write intent rather than add a separate RegisterWrite opcode; platform routing and register permissions remain explicit. A memory context never implicitly grants access to management registers. |
| Fill | Repeat a pattern over a destination range, including zeroing and initialization, without reading a repeated source buffer. |
| Wait | Observe a memory condition without modifying its value. A false condition blocks same-channel successors but releases the shared engine for peers; it is not a PageFault. |
| Atomic | Perform an indivisible memory read-modify-write. Specify the operation set, width, result destination and ordering before encoding. It is not a direct mapping of the HSA software signal API. |
| SEM | Counting semaphore: Signal atomically increments; Wait atomically consumes one permit only when the count is positive, otherwise waits without modifying it. Do not implement it as separate condition Wait and unconditional decrement commands. |
| Fence | Complete prior operations in the specified ordering scope before writing a software-selected completion value to memory. It is an ordered writeback, not an ordinary Write or an atomic shared-counter update. |
| Trap | Explicit notification independent of success-record publication. It does not itself certify prior memory completion; use Fence before Trap when that guarantee is required. |
| Timestamp | Write one 64-bit external timer sample to an authorized target. The harness owns a simulated tick counter; DMA samples directly. Units are platform-defined, not physical RTL cycles. Retain the original sample through target faults and resume only its writeback. |

Software can express `Copy -> Fill -> Copy -> Fence -> optional Trap`. It owns
completion storage and chooses notification/batch boundaries; hardware needs no
batch container. No INDIRECT, COND_EXE, GPU page-table-specific operation, cache
maintenance command or additional copy geometry is selected. PRE_EXE and other
poorly documented SDMA instructions are not inferred requirements.

#### DMA response ring becomes a channel error event ring

The agreed migration retains a per-channel reporting ring, but changes its role
from one response per command to error events only in this slice. Do not silently
replace the ring with a register-only diagnostic scheme.

- Successful commands produce no event. Fence publishes the completion value;
  optional Trap requests notification without adding a success event record.
- A stopping command error records task identity, cause and necessary diagnostics
  in an event. Hardware stops the channel independently of event publication;
  later commands, including success Fences, cannot pass the failed command.
  Preserve channel-reset recovery. Specify which existing diagnostics, including
  committed-byte information, are retained rather than losing them implicitly.
- Recoverable payload PageFault retains the existing MMU report and channel
  suspend/resume contract. It produces no duplicate DMA error event, and no later
  completion Fence can pass the unfinished command.
- Event ring full, corrupt configuration or failed event write is a reporting
  failure, not backpressure or an implicit retry loop. Retain channel fatal state
  and an independent interrupt outlet that do not require a successful event write.
  Fence-write errors also need a reliable reporting path; classify access failures
  explicitly before encoding, rather than assuming every failure is recoverable.
- Consuming an event does not resume the channel, prove resident cleanup or permit
  memory reuse. Reset/Abort must terminate affected software waits without
  fabricating successful Fence values; old writes/notifications must not be mistaken
  for a new allocation's completion.
- Remove mandatory success-response writes, response-space admission gating and
  IOC bookkeeping tied to response consumption. Define Trap, error-event and fatal
  IRQ cause/mask/acknowledgment rules together, preserving notification ownership
  and the existing no-mask-replay rule.
- This is the DMA data-channel ring, not the PF/firmware control response or event
  ring. Keep those management protocols and DMA-wide fatal routing unchanged;
  ordinary channel errors remain owner-visible without PF proxying each task.

AMD's reference separates Fence writeback from Trap notification. GPU IH hardware
aggregates IP interrupts into interrupt rings, which can carry both normal and
error notifications. That is not a dedicated per-SDMA-command response queue or
proof of an error-only ring. PDMA chooses a channel error ring without importing a
GPU-wide IH subsystem. [S3, S4, S12]

#### Atomic and semaphore: model semantics versus RTL mechanism

The functional model executes indivisible operations through generic memory atomic
capabilities. DMA owns SEM policy and waiting state; generic runtime/storage owns
no PDMA opcode or semaphore protocol. Atomicity must cover all participating
accessors to shared backing, not just callers inside one DMA instance. Restore
managed atomic forwarding with the same channel/context authorization as other
accesses; baseline backend atomics alone are insufficient.

RTL may provide three configurations:

| Configuration | Obligation |
| --- | --- |
| Unsupported | Atomic and counting SEM are unavailable, discoverable as such, and rejected explicitly before effects. Ordinary condition Wait and Fence do not require atomic RMW support. |
| Exclusive | Implement operations through exclusive read/write and local retry control, with an end-to-end monitor covering competing accesses. |
| Native atomic | Use supported bus atomic transactions; compound SEM acquisition can still require CAS/retry control. |

AXI4 exclusive and AXI5 AtomicCompare are implementation options, not new descriptor
formats. An unsupported exclusive-read response cannot be treated as a successful
reservation; contention retries and actual bus errors must remain distinct. Native
CAS returns the original value for match detection. Neither mechanism supplies a
complete semaphore wait loop by itself. [S11]

Do not model exclusive monitors, bus cycles or CAS contention retry traffic.
Semantic waiting and lifecycle remain observable: unsuccessful SEM acquisition
does not decrement or block peers; successful acquisition/atomic update is
checkpointed exactly once. Later stalls, notification failures or reset must not
repeat or silently undo a committed update. Drain waits for ordinary dependencies;
Abort/Reset can cancel unfinished waiting work while reset still awaits resident
cleanup. Acquire/release ordering is a separate contract from atomicity and must
hold regardless of the chosen RTL mechanism.

Historical Radeon developer discussion describes counter semaphores and a central
semaphore block, but also contains unresolved multi-ring questions. It is not a
complete specification of every SDMA generation or mailbox mode. Our counting SEM
is an explicit PDMA choice; no dedicated central semaphore block is required. [S10]

#### Specification refinements and acceptance

No further architectural fork blocks this direction. Before assigning wire fields,
record exact operation/width/alignment rules; Wait comparisons and wrap behavior;
SEM overflow and ordering; Atomic returned-data placement; Fence ordering and
visibility scope; access context/lifetime and fault classification for each new
command; event diagnostics and IRQ acknowledgment; and reset/abort completion-value
reuse rules. These decisions and the 2026-10-23 Timestamp/Fence correction are
now recorded in the copy-engine specification; its verification matrix identifies
executable coverage rather than leaving these interfaces as open design questions.
Trace these obligations into the normative spec and generated Host/Firmware ABI,
without presenting planned behavior as already implemented.

Acceptance must include:

1. CPU publication -> Wait -> Copy -> Fence, with explicit visibility requirements.
2. Several commands -> one Fence -> optional Trap, with no success events or hidden
   response-space backpressure; independent channels must not falsely complete one
   another by writing an uncoordinated shared timeline.
3. Contending Atomic/SEM accessors, including two waiters competing for one permit,
   peer progress and discoverable unsupported capabilities in direct/managed paths.
4. Fault/retry and cancellation before/after committed updates, without duplicate
   effects; Drain waits and reset still waits for actual resident cleanup.
5. Stopping errors, diagnostic-event delivery, event overflow/configuration/write
   failure, independent fatal notification, and no successful Fence beyond a stop.
6. Existing Copy/SG, Function/context isolation, page-fault recovery and management
   lifecycle behavior; update obsolete response/IOC tests to the new contract
   without weakening their ownership, cleanup and diagnostic guarantees.

Preserve the existing typed pipeline and command/chunk continuation boundaries.
The final architecture audit must trace model evidence and platform/RTL obligations
separately, using HSA/SDMA as references rather than claiming conformance.

#### Sources and evidence limits

- **S1:** [HSA System Architecture 1.2, sections 2.5, 2.6 and 2.9](https://hsafoundation.com/wp-content/uploads/2021/02/HSA-SysArch-1.2.pdf). Normative HSA semantics, not SDMA implementation.
- **S2:** [ROCr 6.3.3 SDMA builder](https://github.com/ROCm/ROCR-Runtime/blob/637eaec61186cc1f73480d99095ac4a3b2597242/runtime/hsa-runtime/core/runtime/amd_blit_sdma.cpp): `SubmitCommand`, `SubmitBlockingCommand`, `BuildCopyCommand`, `BuildPollCommand`, `BuildFenceCommand`, `BuildAtomicDecrementCommand`. Host code, not hardware execution traces.
- **S3:** [Linux v6.12 SDMA v7](https://github.com/torvalds/linux/blob/v6.12/drivers/gpu/drm/amd/amdgpu/sdma_v7_0.c): `ring_emit_fence`, `ring_emit_mem_sync`, `ring_emit_pipeline_sync`, `ring_preempt_ib`. Generation-specific driver programming.
- **S4:** [Linux v6.12 amdgpu fence lifecycle](https://github.com/torvalds/linux/blob/v6.12/drivers/gpu/drm/amd/amdgpu/amdgpu_fence.c): `amdgpu_fence_emit` and `amdgpu_fence_process`. Software completion bookkeeping.
- **S5:** [ROCr 6.3.3 API reference](https://rocm.docs.amd.com/_/downloads/ROCR-Runtime/en/docs-6.3.3/pdf/): asynchronous copy dependencies, completion signal and engine selection. API contract, not proof of atomic fallback safety.
- **S6:** [Linux 6.16 GPU hardware and queue overview](https://docs.kernel.org/6.16/gpu/amdgpu/driver-core.html#gfx-compute-and-sdma-overall-behavior). Queue/pipe roles, not an instruction-level scheduling specification.
- **S7:** [Linux v6.12 GFX9 SDMA queue destruction](https://github.com/torvalds/linux/blob/v6.12/drivers/gpu/drm/amd/amdgpu/amdgpu_amdkfd_gfx_v9.c#L576-L613). Context idle observation is checked, not assumed.
- **S8:** [AMD AQL command-processor description](https://rocm.docs.amd.com/projects/ROCdbgapi/en/docs-10.0.0/doxygen/html/index.html). Compute AQL processing is distinct from S2's SDMA-native path.
- **S9:** [ROCr API reference, retrieved 2026-10-21](https://rocm.docs.amd.com/projects/ROCR-Runtime/en/latest/api-reference/api.html), `hsa_amd_memory_copy_op_s` and `hsa_amd_memory_copy_signal_op_t`. Moving documentation; newer API example only, not the version baseline for S2.
- **S10:** [Radeon semaphore discussion, Christian Konig, 2011-11-15](https://www.mail-archive.com/dri-devel@lists.freedesktop.org/msg16307.html) and [follow-up questions](https://www.mail-archive.com/dri-devel@lists.freedesktop.org/msg16308.html). Historical counter/central-block explanation, not a universal SDMA SEM or mailbox specification.
- **S11:** [Arm AMBA AXI/ACE specification, IHI 0022H](https://developer.arm.com/-/media/Arm%20Developer%20Community/PDF/IHI0022H_amba_axi_protocol_spec.pdf). Exclusive access and AtomicCompare mechanisms; platform support and atomicity scope must be established, not inferred from an AXI version label.
- **S12:** [Linux 6.17 GPU hardware structure](https://docs.kernel.org/6.17/gpu/amdgpu/driver-core.html#gpu-hardware-structure). IH aggregates IP interrupts into rings; it is separate from SDMA's command and completion-value storage.
- **S13:** [Linux v6.12 SDMA v6 packet definitions](https://github.com/torvalds/linux/blob/v6.12/drivers/gpu/drm/amd/amdgpu/sdma_v6_0_0_pkt_open.h). Native and AQL packet layouts, not proof of deployment or software-path selection on every device.

### Deferred

Full PCIe/GPU-MMU integration is not part of the first discussion. Recovery without
working firmware remains the separate low-priority TBD below. Update the normative
spec and implement focused scenarios only after the relevant semantics are agreed.
NPU development is deferred; add no NPU commands, computational ISA, generic compute
framework or speculative abstractions as part of the stages above.

Reference starting points:

- [Linux HMM](https://docs.kernel.org/mm/hmm.html): page migration and device page-table synchronization responsibilities.
- [Page-fault recovery sources and evidence limits](page-fault-recovery.md#references-and-evidence-limits): Intel partial completion and scoped NVIDIA comparisons; do not combine different architectures into an unverified VF design.

## Implemented preparation: shared drivers and management interrupts

Recorded: 2026-09-17. The two items below are implemented alongside separate Host
and Firmware header views. They prepare the broader feature set without adding
virtualization. Shared DMA control lives in `software::dma`, control cursor layout
and ISR wake events in `software::control`; both domains receive real management
interrupts through their own `InterruptRuntime`.

### 1. Share PDMA software driver capabilities

- Move reusable DMA driver capabilities from `host` into `software::dma`.
  Separate register control (discovery, faults, IRQ control and reset) from the
  queue-owning driver (configuration, submission and error-event consumption).
- Reuse the channel reset request/completion handshake in both Host and firmware.
  Firmware release must not construct a queue-owning `Channel` merely to reset it.
  Preserve Host `Channel::reset(self)` invalidation and each domain's bus permission
  checks without imposing CPU affinity on the driver. Timeout/cancellation is not drain completion.
- Share control-queue layout access in `software::control`, including the repeated
  producer/consumer address calculation and cursor adapter construction. Keep Host
  initialization/request correlation and firmware attachment/service roles distinct.
- Split errors with their owning responsibilities; do not move the whole mixed
  `host::Error` into a new common catch-all enum.
- Keep Host management-client semantics and firmware boot observation in Host.
  Keep allocation, window closure and release/reset sequencing in firmware.
  `host::runtime::Model` is a runtime adapter, not shared software driver code.

Acceptance must show both domains using the same reset handshake, ordinary DMA
remaining usable, real release during a transfer still draining before acknowledgement,
and no software dependency being introduced into hardware or generic framework crates.

### 2. Deliver doorbells through management interrupts

The agreed path is: publish a ring update, write its doorbell, emit a management
interrupt, run the receiving domain's ISR, wake its processing task, then inspect
and process the ring. Ring state remains authoritative; an interrupt is a hint.

- Provide Host-to-firmware and firmware-to-Host management interrupt delivery
  independently of DMA interrupts. Do not reuse DMA causes or its notification vector.
- Host and firmware register their own ISRs as part of their software programs.
  An ISR wakes the processing task; it need not execute management requests itself.
- Preserve notifications for returned queue space as well as published entries,
  so a peer blocked by a full ring can resume. Preserve coalescing and tolerate
  spurious notifications; establish a wait protocol that cannot lose a wakeup.
- Reuse the existing `InterruptRuntime` for each domain's separate software CPU.
  Concrete wiring and scoped actor lifetime stay in the scenario harness. Hardware
  emits interrupts but never stores or invokes software handlers.
- Remove direct use of `wait_notification()` from business software. Generic
  notification primitives may remain runtime-internal implementation tools.

Acceptance must exercise actual ISR delivery in both directions, independence from
DMA interrupts, notification-before-wait races, full-ring space return, cancellation,
and coordinated shutdown. The implementation retains coalesced doorbell pulses:
the platform source's claim consumes pending before ISR dispatch. There are no
additional management cause/mask/ACK registers.
Neither item adds an event ring, Function/PASID support or PCIe integration.

### 3. Separate C access views

The contract projects one physical register schema into Host and Firmware views.
`pdma_common.h` carries shared wire records, with `pdma_host.h` and
`pdma_firmware.h` exposing only each domain's permitted registers and access modes.
Offsets stay unchanged. Generic schema projection/export knows no PDMA roles;
actual hardware permissions are checked against the projections in boundary tests.

## Objective

Use firmware plus the existing shared DMA hardware as the primary architecture
for preparing SR-IOV, PASID and function-level reset support, and evaluate GPU/SoC
copy-engine fault recovery and synchronization at the same functional abstraction level. Keep policy and
management sequencing in firmware, with enforceable isolation and completion
mechanisms in hardware. Do not integrate with `pcie_modeling` yet; establish
boundaries that reduce the eventual integration work.

Preserve the ordinary non-virtualized configuration. It must remain usable without
firmware startup, PCIe services, context tables or external reset handshakes.

## Agreed direction

- Firmware manages Function-to-channel allocation and lifecycle decisions.
  The PDMA core need not understand FunctionId, PF/VF or PCIe management policy.
- Channels retain their rings, registers, execution state boundaries, faults and
  reset mechanisms. Keep one shared DMA execution pipeline, composed using the
  existing building blocks; do not introduce one engine per Function or channel.
- Commands independently select SG-table, source-payload and destination-payload
  contexts. A channel register selects the context shared by its two DMA rings.
  Control memory has independent PF-programmed access configuration. These are
  reusable access bindings, not task identifiers; preserve task ID as a
  submission/diagnostic correlation value. The next DMA reporting ring is an error
  event ring. See the [context contract](context-contract.md).
- Access logic in `hw-components` derives Function identity from the controlled channel
  binding and resolves the selected context to PASID and any required access
  attributes. Lookup must be scoped and authorized; a guessed context ID must not
  grant access to another Function's address space.
- Firmware owns allocation policy; hardware routing/permission configuration
  enforces the resulting decisions. Avoid duplicate mutable ownership tables in
  the PDMA core, runtime and firmware.
- PDMA reports channel events. Interrupt routing in `hw-components` maps them to the
  appropriate Function/vector without putting PCIe notification protocols or
  software interrupt handlers into the core.

## Responsibility boundaries

| Component | Responsibility |
| --- | --- |
| Contract | Software-visible registers, descriptor context selection, status and capability definitions once their semantics are agreed. |
| `dma` | DMA core: channel execution, admission/stop/reset, local completion facts, and memory requests through injected access capabilities. |
| `hw-components` | Hardware components outside the DMA core: authorized MMIO views, channel/context-to-request identity resolution, interrupt routing, and ownership of accepted external access obligations. |
| Device firmware | Resource allocation, management register programming, lifecycle sequencing, and coordination of independently reported completion conditions. |
| Host software | Use granted channels/contexts, allocate buffers, submit commands, observe completion values and error events under the copy-engine protocol, handle interrupts and coordinate OS address-space lifetimes. |
| Runtime/platform | Assemble hardware and software execution domains; provide memory backing, transports, wakeups and interrupt delivery. No PDMA traversal, copy or recovery policy. |

Device firmware is a real modeled software actor with its own execution context.
It accesses hardware through management capabilities/MMIO, not direct mutation of
`Dma` internals. Firmware does not replace the hardware DMA data path.

### Code and assembly boundaries

The agreed target is separate `dma`, `hw-components` and `firmware` crates, with
shared soft/hard definitions in `contract` and Host software interfaces in `host`.
Shared PDMA software algorithms now live in `software`, used by Host and firmware;
that boundary includes rings, DMA drivers and common control-queue access.
`hw-components` is a collection of concrete hardware objects, not a generic bus
fabric or a firmware policy controller. Each object owns its own registers/state.
The core now lives in `crates/dma`; the first access window and firmware program
live in `crates/hw-components` and `crates/firmware` respectively.

Firmware is an async program. Host and firmware reuse the existing software CPU
implementation with separate CPU instances, CPU-local state and capability views.
Each domain now uses its own interrupt runtime for management delivery; Host also
receives DMA interrupts. Sharing an async executor does not mean sharing a
software CPU or granting access to the same hardware view.

A distinct concrete runtime assembly connects and drives the firmware-managed
system. Retain the existing ordinary assembly/transport; do not turn
`host::runtime::Model` into an all-purpose container of optional features. Reuse
memory, CPU, interrupt and pipeline components rather than implementing another
executor or dispatcher. The managed assembly now lives in
`crates/scenarios/tests/firmware_managed/harness/runtime`, with its entry module
at `harness/runtime.rs`. The additional managed-runtime crate has been removed.
Firmware management sequencing and hardware access enforcement stay out of it.

Production firmware and hardware objects live outside `scenarios`. Scenarios
contain cases, acceptance assertions and the harness for concrete system assembly,
external provisioning, startup, fault injection and teardown. Test-specific Host
programs remain test support; reusable Host driver behavior belongs in `host`.
The harness must not implement firmware initialization or hardware behavior.
Both transports reuse the independent IRQ crate's pending-interrupt component.
Memory interfaces, supplied backing and closure storage share the std-only IO package;
DMA consumes its capabilities without software execution dependencies. Hardware owns
schema register windows and access effects; IO has no register-image layer or schema dependency.
SoftwareEnv/main/ISR scheduling stays in the software library. See
[reusable library boundaries](reusable-libraries.md).
A generic shared-context wrapper was evaluated but not introduced: model-specific
projection and IRQ extraction still belong at the consumer, and remaining mutex/wakeup
mechanics do not justify extra hooks or traits.

## Management MMIO

Provide a privileged management view where necessary. Reuse existing channel
register state and operations rather than creating shadow registers.

- Host/VF work views expose only authorized resources and operations.
- Firmware's PDMA management view can control the relevant channels and observe
  stop/reset completion; add only genuinely missing hardware mechanisms.
- PCIe-facing management registers belong to the integration hardware, not the core:
  Function lifecycle requests, access/context routing and controller handshake
  state do not belong in the PDMA core.

Host software uses an authorized work MMIO view for normal ring submission,
error-event consumption and permitted channel/IRQ configuration, without firmware
proxying every command. Firmware has a separate privileged view of management
hardware and relevant channel controls. Both views can reach the same underlying
channel state; neither requires shadow registers. Host accesses cannot bypass the
work-side checks or acquire the management capability. Closing Host MMIO does not
by itself stop already accepted DMA work.

## Lifecycle and memory invariants

The initial design does not support live context rebinding. Before reusing a
channel or context, close old submission/configuration paths, stop or clean up
all old work that can reference it, and drain already accepted accesses. Queued
commands referencing a context count as old work, not just the executing command.
Reconfigure memory, MMIO and interrupt routing before reopening the resource.
Hardware must enforce the relevant configuration restrictions, not rely solely
on firmware convention.

Accepted accesses retain their original identity and necessary data until their
terminal outcome. Dropping a stage or future is not transaction cancellation or
drain. Resolving and retaining an identity does not by itself make early PASID
unbind or backing-memory reuse safe.

Keep these completion facts separate:

1. PDMA local stop/reset completion: old work cannot issue new accesses,
   completion/event writes or notifications.
2. External access drain: accepted obligations have reached their qualified
   completion point, as reported by their owning port.
3. External lifecycle release: controller/integration conditions permit reopening.

Resource-lifecycle sequencing must respect these facts; writing a completion
register cannot fabricate memory retirement. For the agreed VF pre-FLR sequence,
firmware completes PF-requested channel release before FLR, while the PCIe layer
owns external drain and controller release. Firmware does not receive FLR requests.
A stalled firmware cannot establish successful preparation. Firmware and the PF
management channel remain outside the VF reset scope; PF and whole-device reset
still need separately defined recovery paths.
Already platform-pending notifications must not be reassigned to a new owner.

Preserve the synchronous storage backend. Add waiting/request-completion semantics
at the device-facing access boundary only as needed, with an immediate-completion
implementation for ordinary memory. No artificial bus cycles, mandatory pending
tick, allocation prepare/install tokens or duplicate outstanding ledgers.

## Ordinary configuration

Distinguish capabilities absent at instantiation from supported capabilities
disabled by software. The simple configuration can use an implicit default
context, direct memory access, one notification destination and locally completed
reset. It must not require a Function-management table or firmware control loop.
Reuse the same execution stages; avoid feature checks scattered through stages
and speculative collections of policy generics.

## Questions for the broader goal, beyond the MVP

- Context selector and management layouts are implemented in the contract; future
  extensions must preserve the documented ownership and lifecycle boundaries.
- Target-specific context attributes and Device-memory protection; selector roles,
  PF control bootstrap and snapshot-based Drain/Abort semantics are now recorded
  in the [context contract](context-contract.md), not open alternatives.
- Exact management access permissions, stop/rebind interlocks and ownership of
  local-completion, external-drain and release observations.
- Recovery when firmware or the management transport itself is unavailable remains
  deferred; see the low-priority TBD below. PF events and firmware-mediated DMA/event
  recovery are implemented in [fatal and events](fatal-events.md).
- Synchronization follows the agreed command families and Fence/Trap/error-event
  protocol above; field semantics, wire format and Timestamp's external clock
  boundary are specified in `copy-engine.md`.
  Logical-channel scheduling and oversubscription remain
  separate deferred work. Retained physical-channel suspension and owner resume
  remain specified in the page-fault chapter.
- Which features are instance configuration and which have software enable bits.

## Deferred TBD: subsystem recovery without working firmware

Recorded: 2026-09-28. Priority: low. Status: design unresolved; no implementation
scheduled. The user considers this important but not a current architectural blocker.
It is not a missing part of the completed DMA fatal/event recovery feature.

The potential scope is recovery when firmware has stopped responding or the
management command/response transport cannot operate. Existing `ResetDma` and
`ResetEvents` depend on functioning firmware and management transport; they retain
their documented behavior.

Before implementing, decide:

- The recovery trigger and authority: external platform reset, independent MMIO,
  or another integration mechanism; no watchdog or new register interface is agreed.
- Reset domains and retained state, including firmware execution, DMA pipeline,
  windows, context bindings, management queues and platform-pending notifications.
- Completion and memory-lifetime guarantees for old accesses, followed by firmware
  startup, Host re-bootstrap and coordination of affected Functions.

An external reset input and full subsystem reinitialization were discussed as a
candidate, not an approved design. Do not add a reset controller, runtime framework,
or mandatory RTL requirement from this note alone. Revisit the design explicitly
when this item is prioritized; keep hardware, firmware, Host and platform ownership
clear in the resulting specification.

## Future acceptance focus

Retain ordinary copy, SG, fault, reset and interrupt scenarios. Add explicit
hardware/firmware/host/harness separation and test unauthorized resource access,
context isolation, queued-context reuse, stale notifications and unaffected channel
behavior. Page-fault scenarios must cover the retained suspension contract;
synchronization scenarios must cover the selected copy-engine contract after its
remaining interface decisions are resolved. The final architecture review must trace model
evidence separately from platform obligations. External transaction drain and release
remain RTL/platform obligations, not a mandate to lower the model to bus-transaction timing.

## Reference boundary

Reference repository: `https://github.com/shady831213/pcie_modeling`.
Its `docs/architecture/pcie-endpoint-architecture.md` APP-CONTROL section and
`crates/pcie-app/src/control/` distinguish request acceptance, local completion
and manager release. The architecture document records an ongoing migration:
recheck actual interfaces when integrating. Do not restore its legacy centralized
per-work admission/token tables into PDMA. PCIe capabilities, ATS/PRI, MSI-X and
DWC-specific handshake encoding remain outside this preparation goal.
