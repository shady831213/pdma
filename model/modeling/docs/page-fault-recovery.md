# Pageable-copy recovery: retained channel suspension

Recorded: 2026-09-28. Updated: 2026-10-09.
Status: approved payload-only contract, superseding terminal PageFault responses
and software resubmission. This is not a released ABI or a cycle/bus specification.
The [system specification](spec.md) indexes this chapter.

## Two copy uses and two recovery boundaries

Prepared page migration uses ordinary copy under the
[non-faulting migration preconditions](context-contract.md#non-faulting-page-migration-copy).
Its servicing copy must have accessible payload, queue and SG storage. PDMA does
not implement a pager, page-table updates or MMU invalidation.

For pageable virtual-address copies, a backend may explicitly return
`MemoryFailure::PageFault` on a payload access. DMA parks the original command on
its physical channel. The platform MMU/backend reports fault identity, address and
requested access independently; authorized software repairs the mapping and resumes
that channel through its work MMIO. PDMA publishes no intermediate fault response,
DMA interrupt or firmware event. The eventual normal response completes the original
command with its original ID and total committed length.

A transparent recoverable access port is a different boundary. The existing
external ATS+PRI port can retain the original transaction while repair occurs and retry
internally. Such a stall does not return a PDMA PageFault or require PDMA resume.
Connecting that port, implementing a true MMU, and modeling outstanding PCIe
transactions remain outside this change. Do not implement both recovery protocols
for the same held transaction.

## Responsibility boundaries

| Owner | Responsibility |
| --- | --- |
| DMA | Preserve accepted command, both endpoint cursors, committed prefix and any read scratch; park/resume at a replay-safe boundary and serve eligible peers. |
| Contract | Define normal completions, independent endpoint positions and the channel FAULT_STATUS/RESUME bit interface. |
| hw-components | Enforce channel ownership and context authorization; expose the authorized identity and original source to platform sideband construction. |
| Host/VF software | Correlate platform faults with owned work, authorize repair, establish translation visibility, explicitly resume and obey resource lifetimes. |
| Firmware | Retain existing allocation, context maintenance and reset/release sequencing; no per-copy repair service or resume opcode. |
| Platform/MMU/backend | Supply classified access outcomes and independent fault reports, fault-to-channel correlation, translation and repair services. |

The physical channel population is fixed at construction and all channels share
one execution engine. Parking permits another channel to run; it introduces neither
channel oversubscription nor a firmware task scheduler. Ordinary direct-DMA use
continues without firmware, context tables or a pager.

## Agreed decisions and rationale

### PF-D01: independent endpoint positions and one copy length

Source and destination independently select Linear(base, byte offset) or
ScatterGather(table, count, start index, entry-local offset), each with its own
context. One copy_length selects both ranges; backing capacities need not match.
Hardware starts at the selected SG entry and validates only the selected ranges
before payload access. Normal subrange requests still use these positions; they
are not a fault-continuation wire protocol.

### PF-D02: retain committed progress and read scratch

Only a successful destination write advances the committed prefix and both
cursors. A failed PageFault call commits no external writes from that call.
On a read fault, resume retries that read. On a write fault, retain the successful
read's scratch and retry the write without rereading the source. Earlier committed
writes are never replayed. Source and destination SG boundaries may differ.

Overlap remains governed by actual chunk order, without a whole-copy memmove
guarantee. Retained scratch also avoids changing a pending write if its source is
modified while parked. Real RTL must establish equivalent replay-safe commit
boundaries; the synchronous model does not prescribe serial physical transactions.
Ordered task completion does not imply that only one memory transaction may be
outstanding. Parallel physical execution would need equivalent commit/retirement
accounting; it is not implemented here.

### PF-D03: owner resumes the original physical channel

The owner correlates a platform fault, observes FAULT_STATUS.faulted, repairs the
mapping, then writes RESUME.request=1. That request clears faulted and makes
the retained work eligible under ordinary enable/error/admission checks. It does
not create a new command, enable a disabled channel or clear an error/fatal state.
The channel's later commands remain blocked until the original command completes
or is safely cancelled. Other channels may run while it is parked.

### PF-D04: the platform owns fault information and repair

The platform fault report must identify the address space, failing address and
requested access, and support correlation with the suspended channel/incarnation.
A failed access may span a boundary, so its faulting address can differ from the
access start. FAULT_STATUS contains only a faulted bit; DMA responses carry no fault record.
Hardware channel/context routing still owns binding checks. PASID, page-table
formats and page sizes remain outside DMA.

PageFault means explicitly repairable not-present payload access. Unmapped model
storage, allocation failure, authorization rejection and ordinary permission denial
must not be silently relabeled as paging. A fault report does not authorize wider
permissions. Additional classes such as repairable copy-on-write require an explicit
classification agreement. Repair must make mappings visible and perform required
translation-cache invalidations before resume; a PTE store alone is insufficient.

The platform attaches an opaque channel-source tag (`ChannelTag`, carried as
`ch_tag` in the scenario platform) to the outgoing request and the MMU
returns it unchanged with the fault report. Function/PASID identifies an address
space, not a physical channel: two channels may use the same PASID and VA. Software
must use the platform's tag association, not infer the channel from those values.
DMA continues to emit only its channel/context selectors. Neither DMA nor the MMU
defines the platform tag's encoding or lookup policy. The resolved MemoryIdentity
remains an address-space identity, independent of the tag.
The name describes the source association, not a PDMA channel-index encoding or
a fault-episode token. Context attributes and the channel tag remain separate:
multiple channels may share one context, and one channel may use several contexts.

### PF-D05: suspension is neither completion nor a stopping error

No response slot, IOC/non-empty cause, response_error, channel fatal or PF event
is published merely because payload work parks. Accepted task/context references
remain live. Metadata and response-publication failures retain their existing
stopping/fatal handling. Repeated faults set faulted again and require another
explicit repair/resume; there is no automatic DMA retry loop.

### PF-D06: cleanup includes parked work

Drain cannot succeed while captured work is parked. It returns ChannelStopped
failure with the affected channel bitmap so firmware can process later recovery
requests. Software may repair/resume and retry maintenance, or reset before retry.
Abort retains command-atomic behavior for running work, but discards matching
parked work without a response; this also applies if running target work parks
after Abort was accepted. Reset cancels parked work without waiting for resume.
Both cancellation paths preserve committed writes and retire accepted references
before acknowledging cleanup. Bindings remain valid on failure.

## Normative requirements and wire details

**HW-PF-01 (range acceptance).** The current tagged Command is 112 bytes:
id/flags/opcode/payload at 0/8/12/16. For Copy, source/destination/copy_length/
sg_context are at absolute command offsets 16/56/96/104. Each 40-byte endpoint has
mode/context at 0/4 and geometry at 8: Linear has base/offset; SG has
table/count/index/offset. All records are little-endian, eight-byte aligned;
unused storage encodes zero. Generated schemas are the wire-layout authority.

For nonzero length, Linear selected address/end arithmetic must fit u64. SG starts
directly at its index, skips zero-length entries and walks only far enough to cover
the selected length. Offset beyond an entry or selected table-address overflow is
InvalidLength; offset equal to an entry length advances. Insufficient entries,
including start at/beyond count, is LengthMismatch. Inaccessible or undecodable
entries are ListReadError. Neither unused prefixes nor suffixes are scanned.
Both ranges pass before payload access; zero length touches no list or payload.
Typed endpoint positions and matching-variant `with_start` remain general range
selection helpers, independent of fault recovery.

**HW-PF-02 (progress and final response).** Retain the original command, both
current SG/Linear cursors, committed prefix, current copy phase and successful read
scratch in the Channel hardware object while parked. The next hardware action and
the results of completed transforms are explicit state. A write fault resumes Write
using the already transformed bytes, not Read or Transform. Cursors alone are
insufficient when a transform has side effects or is not idempotent. Do not store
the Rust composition's invocation state as a substitute for hardware state; retry
only the failed access after authorized resume.
Response is 24 bytes: id/status/flags/info at 0/8/12/16. Copy info contains u64
bytes_completed. Copy status values 0..5 retain their existing success/stopping-error
meanings; former PageFault status 6 is invalid. SignalSuccess 7 and SignalError 8
use the separate result payloads defined in [Signal/Wait](signal-wait.md).
PageFaultInfo, fault access and
continuation-position response fields are removed. A final success reports the
original copy_length; a later ordinary error reports the original command's
committed prefix. Completion uses normal publication/notification rules.

**HW-PF-03 (channel lifecycle).** A parked channel accepts no successor command.
Parking releases shared execution capacity without retiring accepted ownership;
terminal completion or cancellation cleanup releases it. Drain/Abort account for
all parked references as well as currently executing work. Reset guards precede
every old-work access. Metadata and response faults do not use the resume protocol.

**HW-PF-04 (fault/resume MMIO).** `ChannelRegisters.fault_status` uses the read-only
four-byte `FaultStatus` schema with boolean `faulted` at bit 0.
`ChannelRegisters.resume` uses the write-only four-byte `Resume` schema with boolean
`request` at bit 0. A payload PageFault sets faulted. Writing request=1 clears the
currently exposed faulted bit and permits scheduling under ordinary channel checks;
it neither enables the channel, clears stopping/fatal errors nor completes a task.
Writing request=0 does nothing. A request while not faulted does nothing and cannot
prearm a later fault. Another PageFault sets faulted again and requires another
request. The schema's owner/window, width and reset-busy access checks apply.

There is no fault-episode identifier, sequence allocator, overflow fatal, reset
preservation counter or hardware rejection of arbitrary late/duplicate requests.
A late request arriving during a different fault clears that current fault.
Reset and applicable Abort discard retained work and clear its faulted state;
software must meet HOST-PF-02 before resource reuse.

**PLAT-PF-01 (classification and atomic failure).** Backend errors remain backend
owned; adapters preserve them. DMA classifies through `Into<MemoryFailure>` only
at the payload boundary. A PageFault write commits no external byte from that call.
RTL adapters must preserve that property or refine the internal commit boundary
without replaying visible writes. Platform fault notification and identity/address/
access correlation are independent of PDMA IRQs and completions.

**PLAT-PF-02 (opaque source association).** After hardware binding authorization,
the platform constructs the outgoing sideband from the original DMA source and
resolved address space. The MMU preserves the opaque tag in its fault report; the
platform resolves it to the correct DMA instance and physical channel. Tags are
trusted platform metadata, not software-selected descriptor permissions. Tag
construction must not bypass owner/context checks or substitute for them.

The platform may encode a stable source directly or maintain an association table.
It must not reuse a tag for a different source while old accesses, queued reports
or handlers can still refer to it. The association scope includes device instances
and their lifetimes. Unknown, foreign or retired associations must not cause a
repair or resume on another channel. A stable physical-channel association does
not identify a grant or fault episode: software must still obey HOST-PF-02 when
that channel is reset, released or reassigned. This does not introduce a DMA ticket,
a hardware tag allocator, or a universal tag width/wire encoding.

**HOST-PF-01 (repair and resume).** Retain request/channel/context/address-space
associations, correlate the platform report and faulted channel, authorize repair,
establish mapping/TLB visibility, then write request=1 through owned work MMIO.
The shared software interface exposes `faulted() -> Result<bool, _>` and `resume()`
without an argument. Observe faulted again after another fault; handle repeated/no-progress faults by
software policy. Do not resubmit the original or residual command to recover it.
No automatic pager is added to submission, runtime or firmware.

**HOST-PF-02 (ordering and lifetime).** Keep descriptors, SG storage, payload
resources and bindings stable through final completion or confirmed cancellation
cleanup. Serialize repair/resume with context maintenance, reset, release and
reallocation. Before reuse, retire old fault handlers and their mapping edits and
pending resume requests, including cross-CPU/PF/VF actors. Prevent duplicate requests
for an already serviced episode from reaching a later episode. This is an owner
software obligation; hardware cannot identify stale fault reports or late resumes.
Use platform ordering/completion mechanisms where required: ordinary ordered MMIO
from one issuer does not by itself serialize independent actors or drain all paths.
Stop new target submissions before maintenance and preserve exclusion during
recovery. Failed Drain, timeout or cancelled wait never authorizes reclamation.
See context SW-02/SW-04/SW-09/SW-10.

**FW-PF-01 (resource boundary).** Keep existing context and channel lifecycle
operations. Destroy awaits successful Drain then explicitly clears valid. Return
ChannelStopped and retained failure information when parked work prevents Drain;
do not block the serial control queue waiting for repair. No new resume opcode,
page-fault event, VF management queue or PF proxy requirement is introduced.

**MODEL-PF-01.** Keep bounded per-channel task state and one shared execution
pipeline. Shared execution stages borrow only the selected channel and required
memory/lifecycle resources, and exchange actual typed chunk/payload/completion
values with guarded identity. A fault ends the finite dataflow, not the
accepted task; a later candidate derives runnable work from Channel state.
No saved generic child continuation or per-channel engine is required.
Ordinary stages prepare a chunk and pass its data through Read/optional Transform/Write.
The finite chunk pass may complete without retiring the command. Shared execution
occupancy backpressures admission until terminal retirement or fault, so healthy
chunks do not acquire another grant or refetch the descriptor. Admission may still
advance empty candidate sweeps without changing the selected owner or RR state.
Even early RESUME requires a new grant before
more payload access; faulted task state remains in Channel, not a combinator.
Fresh command validation is not repeated on resume. ReadCopy/WriteCopy consume
their own concrete checkpoint after regrant. Retained write payload is not sent
through Read/Transform again; those stages may receive empty bubbles before the
same write/publication path consumes the saved bytes. No global controller selects
the failed stage or jumps into a nested graph.
Reuse existing pipeline/runtime components. Inject test page
presence and fault reports through the platform backend; these are not a true MMU.
Scenario software owns allocation, fault handling, repair and MMIO resume. Memory
calls remain synchronous within a deterministic step, without bus timing.

## Decision history and future boundaries

The detailed [decision record](page-fault-decisions.md) explains the two copy uses,
retired terminal partial completion, retained operation boundary, public GPU
comparisons, rejected tickets, and future logical-channel/firmware limitations.
That record is rationale; the numbered requirements here define the contract.
Signal dependency waits, ordered visibility and atomic fault recovery now have a
separate [contract](signal-wait.md). Atomic PageFault commits no update and uses
the same platform report/owner RESUME path. An ordinary false Wait is not a fault,
needs no RESUME and leaves Drain pending until its dependency resolves.
No AQL barrier opcode, implicit fence, channel oversubscription,
true MMU or PCIe integration follows from suspension.

## Verification obligations

These are required scenarios, not a report that tests have passed:

- Wire/header consumers: 112-byte tagged command, 24-byte final response,
  Copy statuses 0..5 plus Signal statuses 7/8 (retired 6 invalid), and
  four-byte read-only FAULT_STATUS/write-only RESUME with boolean bit 0 and
  identical physical placement in role views.
- Linear/SG/mixed copies: read/write and interior-address faults, no intermediate
  response/IRQ, repeated faults, no committed-prefix replay, and retained write
  scratch even if the source changes while parked.
- Scheduling: later work on the same channel blocks; peer channels finish;
  write-one resume continues the original ID; zero/no-fault writes do not prearm;
  completed non-idempotent transforms are not repeated on resume.
- Lifecycle: reset/regrant and device soft reset clear faulted and cancel retained
  work; reset needs no resume; Drain returns failure/bitmap; Abort discards parked targets, including
  a target that parks after acceptance, without a response or stale later access.
- Managed software: independent platform fault reports resolve distinct PASIDs at
  identical VAs; authorized VF work MMIO resumes; foreign owners are rejected;
  firmware continues control service and releases resources only after cleanup.
- Source correlation: channels sharing a Function, PASID and fault VA remain
  distinguishable by opaque platform tags. Resuming the reported channel must not
  resume its parked peer; unknown/foreign/retired associations cannot select work.
- Hardware routing: platform sideband construction occurs only after authorization
  and preserves typed downstream errors on validate, read and write. Untagged
  direct and managed assemblies retain their existing behavior.

Contract, DMA and firmware-managed scenario suites own these checks. Real mapping
updates, invalidation, physical commit boundaries, owner fault-handler retirement
and outstanding-transaction drain require RTL/platform verification.

## References and evidence limits

Vendor evidence motivates design; it is not an assertion that PDMA reproduces a
vendor's full firmware/RTL architecture. No NVIDIA hardware execution was tested.

- [Intel DSA User Guide, section 4.1.5](https://cdrdv2-public.intel.com/759709/353216-004-intel-data-streaming-accelerator-user-guide.pdf#page=26)
  motivated the superseded software-continuation design. Intel has other fault
  policies; the historical analogy covered only partial completion.
- [NVIDIA UVM, pinned source](https://github.com/NVIDIA/open-gpu-kernel-modules/blob/61dcc93722ecb418bb5f2e00923f05b4b8051dd1/kernel-open/nvidia-uvm/uvm_gpu_non_replayable_faults.c)
  shows page repair followed by clearing faulted state/rescheduling the original
  channel. Its MEMOPS path targets the faulted channel from another channel;
  direct register clearing is another configuration. This does not establish
  firmware reconstruction of a residual copy or recovery on the original work FIFO.
- [NVIDIA copy-engine fault-and-switch patent US20180314431A1](https://patents.google.com/patent/US20180314431A1/en)
  describes saving copy methods and replaying from the faulting copy operation,
  while the engine can switch work. This differs from preserving every pipeline
  transaction or PDMA's exact retained scratch boundary. Its pipelined reads also
  illustrate why ordered copy completion does not imply serial transactions.
  A patent does not prove deployment on a particular GPU.
- [NVIDIA RISC-V Summit slides](https://static.sched.com/hosted_files/riscvsummit2024/fe/Keynote_2024.pdf#page=11)
  show GSP/RM and guest-associated partitions, not a combined VF copy/reset FIFO.
- [Turing PBDMA](https://github.com/NVIDIA/open-gpu-doc/blob/9fdf5c4062007929d9f4e6cbad9c9771fe61b880/manuals/turing/tu104/dev_pbdma.ref.txt)
  and [pushbuffer encodings](https://github.com/NVIDIA/open-gpu-doc/blob/9fdf5c4062007929d9f4e6cbad9c9771fe61b880/manuals/turing/tu104/dev_ram.ref.txt)
  distinguish command dispatch from payload execution; they do not prove the
  complete current GSP/VF interface or checkpoint implementation.
- The etha_model RTL implementation was discussed as structural evidence for
  independent SG entry/local-offset range selection, not recovery or measured
  area/timing. This source is not a public verification artifact in this change.
- [Linux page requests](https://github.com/torvalds/linux/blob/v6.12/include/linux/iommu.h#L42-L74)
  and [SVA coherence](https://docs.kernel.org/arch/x86/sva.html) support the identity,
  permissions and invalidation boundary, not a modeled PCIe PRI implementation.

The exact division of GPU checkpoint/save/restore between fixed logic and firmware,
and the end-to-end guest/vGPU/CE transport, remain unresolved. Neither pure-RTL-only
preservation nor firmware residual-task resubmission follows from these sources.
