# Pageable-copy recovery: DSA-like direction

Recorded: 2026-09-28.
Updated: 2026-09-29.
Status: implemented payload-only model contract, with normative requirements below.
This is not a released ABI or a cycle/bus specification. The decision history records
why this design was selected; the [system specification](spec.md) indexes this chapter.

## Two distinct copy uses

Prepared page migration uses the existing ordinary-copy behavior with the
[non-faulting migration preconditions](context-contract.md#non-faulting-page-migration-copy).
An original CPU/GPU fault may trigger migration, but its servicing copy must have
accessible payload, queue and SG storage. This does not implement an OS pager,
page-table updates or MMU invalidation.

The pageable-copy mode concerns copying pageable virtual addresses whose own accesses
can fault. It adopts the partial-completion and software-continuation pattern,
not Intel's register layout, submission portal or complete feature set. Intel's
non-blocking source/destination fault handling provides the relevant precedent [1].

## Selected direction

1. Host/VF software submits through its existing authorized DMA command ring.
2. On a specifically classified recoverable access fault, DMA stops that hardware
   command and publishes a response describing the fault, reliable progress and
   the independent source/destination continuation positions.
3. After publication and normal retirement, that hardware command is terminal.
   DMA retains no suspended task waiting for resume and can service later work,
   subject to ordinary admission and response-space constraints.
4. Host software arranges page/mapping repair through its OS/platform, constructs
   the remaining copy and submits it as a new command. Repeated faults repeat this
   process; policy may instead fail or abandon the logical operation safely.
5. Software correlates these hardware attempts with the original logical task and
   decides when that logical task is complete. Partial completion is not success.

PageFault is distinct from ReadError, WriteError and ListReadError. Those stopping
errors keep their existing behavior; ring-access faults keep the fatal path. A
backend must explicitly report a repairable translation miss rather than inferring
one from inaccessible model storage.

## Responsibility boundaries

| Owner | Responsibility |
| --- | --- |
| DMA hardware | Stop at a defined side-effect boundary, report authoritative progress and fault details, publish the terminal response and retire the command. No page repair or implicit retry. |
| Contract | Encode the agreed endpoint positions and recoverable outcome with diagnostic/progress information. The schema fixes wire layout and exported descriptions. |
| hw-components | Preserve channel/context authorization and access identity routing; do not move binding lookup or Function policy into DMA. |
| Host software | Repair through platform services, build continuation commands, maintain logical-task identity/dependencies and obey context lifetimes. VF need not forward every fault through PF/firmware. |
| Firmware | Keep existing resource and lifecycle management. No per-copy continuation state, new VF control queue or proxy submission requirement. |
| Runtime/platform | Supply memory capabilities and explicitly classified access outcomes; OS/MMU owns actual mapping repair. No PDMA continuation policy in runtime or hidden harness behavior. |

Backend allocation failures, unmapped model storage, permission denial, invalid
context binding and recoverable translation faults are not automatically the same
condition. The injected access-result contract does not implement an
IOMMU or lower the model to bus cycles and outstanding transactions.
Direct, context-free operation must remain usable without firmware or a pager.

## Agreed decisions and rationale

PF-D identifiers retain decision rationale. Normative HW/HOST/PLAT requirements
and executable coverage are listed after the decision history.

### PF-D01: independent endpoint positions and one copy length

Each endpoint retains its own context and independently selects Linear or SG.
The submission fields are:

| Endpoint | Addressing and starting position |
| --- | --- |
| Linear | Base address and byte offset relative to that base |
| SG | Table address, total entry count, start entry index and byte offset within that entry |

The command supplies one copy_length. Both selected ranges must cover that length;
the original source/destination list capacities need not match. This replaces the
previous whole-endpoint-total equality rule. Preserve validation before payload writes, restricted to the
selected ranges; do not scan entries before start_index to rediscover the start.

Ordinary submissions start at index/offset zero. For an uncommon subrange request,
software computes each start. Regular layouts can permit arithmetic indexing;
irregular lists remain supported and may require software traversal. Regularity
is a workload expectation, not a hardware precondition. Hardware directly fetches
the selected SG entry, adjusts its first address/length, then walks forward.

Rationale: expose a useful range selection without requiring hardware to search
an entire SG prefix from a global byte offset. Source and destination positions
cannot be shared because their segment boundaries may differ.

### PF-D02: committed destination progress is authoritative

bytes_completed counts the continuous logical prefix successfully written during
this hardware command. Source and destination positions advance by the same byte
count only after destination write success. Each side independently changes SG
entry when it reaches its boundary. Chunk length is bounded by both current
segments, remaining copy_length and implementation scratch capacity.

For the functional access contract, a memory call returning PageFault must not
leave a committed external write from that failed call. A successful source read
followed by a destination PageFault does not count as completed progress; discard
the scratch and reread that source range on continuation. Earlier successful
writes remain committed. RTL/integration must provide equivalent truthful progress
accounting; this does not require cycle-atomic arbitrary-length bus operations.

Overlap remains software-owned. Do not add hardware overlap rejection or assume
that repeating the whole copy is safe. Software must ensure that rereading the
remaining source after a fault preserves its intended operation.

### PF-D03: PageFault response supplies both continuation positions

| Response information | Meaning |
| --- | --- |
| id | Correlates this hardware command with retained software request state |
| status = PageFault | This attempt is terminal; the logical copy is incomplete |
| bytes_completed | Committed prefix relative to this attempt, not the original logical task |
| fault_address | First virtual byte address causing the failed access to fault, not a physical/PTE address |
| fault_access | SourceRead or DestinationWrite for the first payload-only slice |
| source_next, destination_next | Independent first-uncommitted positions, ready to copy into a new command |
| flags | Existing completion flags, including IOC |

A Linear next position is a byte offset relative to the unchanged endpoint base.
An SG next position is an index relative to the unchanged table plus an entry-local
offset. These are positions, not deltas or opaque resume tokens. Their type follows
the original endpoint. Only PageFault promises these continuation positions;
other statuses must not be interpreted as carrying a resumable checkpoint.

For example, source segments of 3 KiB and 5 KiB and destination segments of 2 KiB
and 6 KiB reach different positions after committing 4 KiB: source index 1/offset
1 KiB, destination index 1/offset 2 KiB. Both describe the same committed prefix.

Software normally computes subrange starts itself. On PageFault, hardware already
knows the actual progress, so return the two positions rather than making software
rescan both lists or rebuild SG tables. Software copies these positions, subtracts
bytes_completed from this attempt's copy_length and resubmits after repair.

### PF-D04: repair information and address-space identity

Continuation positions answer where to restart; they do not identify the page to
repair. Software uses fault_address and fault_access for repair, not a cursor or
an address difference. A failed access may span a boundary, so its fault address
need not equal its starting address or the first uncommitted byte.

The address-space lookup is explicit:

    response.id -> retained command -> source/destination context
                -> retained context/address-space binding -> repair service

No duplicate PASID, FunctionId, page-table root or PTE address is required in the
DMA response under that retention contract. The ID is a command ID, not a context
ID; multiple commands may share a context. Software must retain the request and
binding association, and must not resolve an old response through a recycled slot.
Context ownership/lookup remains in hw-components; DMA does not gain platform
address-space internals.

The initial PageFault category is explicitly repairable not-present payload access.
It must not be inferred from arbitrary Unmapped, context-authorization or permission
errors. SourceRead requests readable mapping; DestinationWrite requests writable
mapping. The OS/platform decides whether access is permitted and repair possible;
the report does not authorize widening permissions. Recoverable write protection,
such as copy-on-write, would need an explicitly agreed classification extension.
Do not add guessed reason codes that the access provider cannot report.

The repair owner knows the page-table format and page size. Before resubmission it
must establish mapping visibility and any required translation-cache invalidation;
writing a PTE alone is not universally sufficient. Linux's page-request interface
similarly separates address-space identity, address and requested permissions [6].

### PF-D05: recoverable completion is not channel halt

The first slice permits recoverable payload faults in Linear and SG copies.
Command/response storage and SG metadata remain accessible for their required
lifetimes. Failures there retain existing handling; response-write failure cannot
report itself through the failing response path.

PageFault publication uses ordinary response non-empty/IOC notification semantics,
does not latch the stopping response-error cause, and needs no special register,
interrupt cause or PF fatal event. Other error/fatal classes retain their policies.
After normal publication/retirement, later work can proceed under existing admission
and backpressure. There is no automatic fault merging or hardware retry loop.

### PF-D06: software owns repair, retries, ordering and lifetime

Hardware stores no task or SG cursor across independent commands. A continuation
is self-contained new work, not restoration of the previous queue position.
Software tracks the logical operation across attempts, decides retry/no-progress
policy and prevents dependent consumers from treating a partial result as success.
Do not hide a pager or automatic retry in the ordinary submit method or runtime.

Keep SG descriptions, remaining payload resources and bindings stable while a
continuation may use them. Command retirement releases hardware references, but
does not finish a software-held logical operation. Existing context submission
exclusion applies to retries. Software must coordinate pending repairs/retries
with Drain/Abort/Destroy, channel reset and release; hardware cannot count software
intent that has not been submitted.

The context remains associated with its original address space until controlled
destruction, not an autonomous hardware expiry. Before requesting destruction,
software stops new submissions, including continuations. Firmware drains, clears
valid, then acknowledges success; failure preserves the binding under the existing
contract. Pending destruction does not authorize new accesses merely because a
binding is still retained for old work. Timeout/cancellation is not release, and
successful Destroy must not be followed by a stale continuation or repair of a
reallocated identity. These rules extend, rather than replace, SW-02/SW-09/SW-10
in the [context contract](context-contract.md#software-obligations).

If a future firmware-managed logical channel owns the request records, firmware
can perform the same ID-to-context lookup and forward the resolved identity,
address and access type to the authorized repair owner. That changes who retains
the association, not the DMA fault-reporting mechanism. It is not implemented or
required by this slice, and firmware does not become the OS page-table manager.

### Decision history: alternatives and why they changed

| Considered approach | Decision and reason |
| --- | --- |
| Report only bytes_completed; software rebuilds both remaining SG lists | Sufficient in principle, but discarded for fault continuation because it requires repeated traversal and may allocate/copy descriptor tables. |
| One command-wide global start_offset | Not selected. It avoids software reconstruction but forces prefix search and over-specializes source/destination selection to the same logical offset. |
| Independent global byte offsets in the two SG streams | Not selected as the SG addressing primitive. Ordinary subrange requests are uncommon; software can locate their starting entries, often cheaply for regular lists. |
| Linear offset; SG start index plus entry-local offset | Selected. Self-contained range selection with direct SG entry access; source/destination remain independent. |
| Software calculates every continuation position | Normal submission uses software calculation; PageFault response instead returns the positions already known by DMA. |
| Hidden cross-command SG cursor or resume token | Rejected for this slice. It complicates reset, scheduling and lifetime and is unnecessary with explicit positions. |
| Duplicate resolved address-space attributes in every response | Not required under retained ID/request/context bindings. Keep the response independent of PASID/platform evolution and forward resolved identity at the software owner when needed. |
| Firmware rebuilds every residual task | Deferred. DSA-like Host continuation preserves the current direct data path and avoids a new firmware task scheduler. |

Offset has non-recovery uses: etha_model's IPsec descriptors locate packet fields
within logical data streams, with separate input/output layouts [5]. PDMA is a
general DMA, so it adopts independent range positions rather than IPsec-specific
field layouts or relative-gap semantics.

According to etha_model's RTL implementation, offset/size selection can be handled
in SG preprocessing with comparisons, subtraction, address adjustment and bounded
state while reusing ordinary alignment support. This is structural evidence, not
a measured area/timing claim. Prefix-search latency still grows with skipped SG
entries. Retaining a cursor between internal requests of one accelerator operation
does not justify hidden state across independent PDMA commands.

## Normative requirements and wire details

**HW-PF-01 (range acceptance).** Command has one u64 copy_length and independent
40-byte endpoints. Each endpoint has mode/context at 0/4 and one union at 8:
Linear { address: u64, offset: u64 } or ScatterGather { list_address: u64,
count: u64, index: u64, offset: u64 }. The same tag selects geometry and start;
there is no second independent position tag in a command endpoint.
Command is 104 bytes: id at 0, source at 8, destination at 48,
copy_length at 88, flags at 96, sg_context at 100. All records are little-endian,
eight-byte aligned. Unused union storage and alignment padding encode as zero.
Generated schema is the wire-layout authority.

Linear has no index field. For nonzero copy_length, arithmetic for base + offset
and the selected exclusive end must fit u64. SG directly reads start.index, skips
zero-length entries and advances only far enough to cover copy_length. A selected
offset beyond its segment is InvalidLength; an offset exactly at its end advances
to the next entry. Running out of entries, including a start at/beyond count, is
LengthMismatch. Selected table-entry address overflow is InvalidLength; an
inaccessible or undecodable entry is ListReadError. No prefix entries are read;
unused suffix entries and capacity outside the selected range are not validated.
Both endpoint ranges must pass before any payload access. Zero copy_length ignores
endpoint starts and performs no list or payload access.

**HW-PF-02 (progress and terminal response).** The first payload PageFault terminates
the attempt. Only successful destination writes advance bytes_completed and both
positions. Response is 72 bytes: id/status/flags are at 0/8/12, followed by
a status-selected info union at 16. Success and stopping-error cases contain only
u64 bytes_completed; their unused union storage must be zero. PageFault uses a
56-byte info record: bytes_completed/fault_address at relative 0/8; access,
source mode and destination mode are adjacent u8 values at 16/17/18; source and
destination position unions start at 24/40. The five alignment bytes are zero on
encode. Each position union stores Linear { offset: u64 } or ScatterGather {
index: u64, offset: u64 }. The unused Linear word must be zero.
PageFault status is 6; SourceRead is 1 and DestinationWrite is 2. There is no
None access in a PageFault. Fault-only information does not exist in other cases.
Unknown status, access or position tags are rejected by the generated codec.

The standalone Position is a self-describing 24-byte tagged union. Compact fault
positions convert to/from it without retaining another command's state.
Endpoint::with_start accepts only a matching position variant and preserves
backing geometry/context; mismatch returns None. A Linear continuation is an
absolute byte offset relative to that endpoint's unchanged base. At an SG segment end, increment index and set
offset zero without fetching the next entry; following empty entries may be skipped
lazily before the next payload access. Positions must never require hidden state
from the retired command. See PF-D02/03 for retry and overlap semantics.

**HW-PF-03 (channel lifecycle).** PageFault uses ordinary response publication,
non-empty/IOC conditions and masks. It does not set response_error, channel fatal
or a firmware event. Later independent commands may proceed without repair or reset,
subject to response space. RunCommand retires accepted context references on terminal
completion; Drain/Abort frontiers do not include software-held continuations.
Reset observed before the next access discards old work, including a pending fault
response, and retains earlier external writes. Metadata and response-write failures
retain their existing stopping/fatal outcomes.

**PLAT-PF-01 (access classification and atomic failure).** MemoryAccessError::PageFault
means a specifically repairable not-present translation. Its address identifies the
first missing byte in the attempted access, possibly after its start. A write
returning that error must commit no externally visible bytes from that call.
Unmapped storage, binding/authorization failure, permission denial and allocation
failure must not be relabeled as paging. DMA does not dictate page size. An RTL
adapter must either prevent a failed modeled access from partially committing or
refine internal commit boundaries so published progress remains truthful; replaying
already-visible writes is not an acceptable substitute.

**HOST-PF-01 (repair and resubmission).** Software retains the command-ID association,
independent endpoint contexts and binding-to-address-space association. It resolves
the access role and fault_address through those retained records, authorizes repair,
makes translations visible (including required invalidations), then replaces both
starts and subtracts this attempt's bytes_completed from this attempt's copy_length.
It may submit a new self-contained command or terminate the logical operation.
Zero progress and repeated faults require explicit software retry/termination policy.
No automatic retry is added to Channel::submit.

**HOST-PF-02 (ordering and lifetime).** Retain lists, remaining payload and identity
bindings until the logical operation completes or is abandoned safely. Do not
release/recycle a context while a repair or continuation may still use it.
Drain success proves the captured hardware work retired, not that a software-held
logical copy completed. Submission exclusion covers retries; no continuation may
race Destroy, reset or release. Dependent work must not interpret PageFault as
successful completion. See PF-D04/06 and context SW-02/SW-09/SW-10.

**FW-PF-01 (resource boundary).** Firmware retains existing Create/Drain/Abort/Destroy
and channel lifecycle responsibilities. It does not repair pages or proxy DMA retries.
Destroy still awaits successful Drain before explicitly clearing valid. Software
must coordinate HOST-PF-02 before requesting it; a failed operation preserves the
binding. No new firmware opcode, VF queue or page-fault event is introduced.

**MODEL-PF-01.** The model keeps one current segment per endpoint and bounded
scratch storage inside the existing Then/Repeat execution. CheckRanges performs
incremental selected-range preflight. Platform test page-presence state is injected
through MemoryAccess; it is not a PDMA page-table implementation. Firmware-managed
scenario software alone allocates buffers, handles DMA interrupts and decides repair
and resubmission.

## Verification links

- contract tests records, scatter_gather, page_fault_wire, descriptions and strict
  C11 header_c consumers pin the new wire layout, codecs and exported documentation.
- dma tests ranges exercise direct SG start, clipped final range, invalid ranges
  and skipped metadata. scatter_gather and scatter_gather_faults retain ordinary
  copy, non-page-fault, overlap and zero-length checks.
- dma tests page_fault cover Linear/SG/mixed endpoints, source/destination misses,
  interior fault addresses, repeated zero-progress fault, no prefix replay,
  notifications, metadata failures, unaffected work and reset before publication.
- scenarios/tests/firmware_managed/cases/page_fault.rs drives actual firmware,
  contexts, channel MMIO, DMA stages and software ISR/repair/resubmission. Source
  and destination share VAs but use distinct PASIDs; Drain succeeds after a failed
  attempt, and bindings are destroyed only after the logical copy and channel release.
- Existing reset, context-maintenance, multi-channel and isolation suites remain
  applicable. Real PTE manipulation, TLB/cache coherence, OS authorization and RTL
  outstanding-transaction accounting require integration verification.

## Deferred device-managed alternative

We explored a per-VF logical task interface accepting copy and lifecycle/recovery
requests, with firmware owning physical DMA submissions. It could hide partial
hardware attempts behind one logical completion, but would introduce task-state,
scheduling, resource-lifetime and notification responsibilities into firmware.
That is not the selected next slice, nor a proven reconstruction of NVIDIA.

A logical queue is not necessarily a firmware-parsed queue. A unified entry
envelope does not imply identical engine capabilities, and a shared service does
not imply one FIFO. Hardware can parse/dispatch a logical submission stream while
firmware manages it. If recovery/control shares an intake interface, it must still
make progress when work is faulted or admission resources are full; placing resume
behind its blocked task is insufficient. A separate recovery queue can still use
the same command framework and target the original channel.

## References and evidence limits

Checked: 2026-09-28; references [5]-[6] added 2026-09-29.
Vendor-specific facts below motivate questions, not mandatory
PDMA interfaces. Different generations and virtualization modes must not be joined
into one assumed architecture. No NVIDIA GPU/firmware execution was tested.

- **[1] Intel DSA User Guide, 353216-004US, section 4.1.5, pages 26-27.**
  [Partial completion handling](https://cdrdv2-public.intel.com/759709/353216-004-intel-data-streaming-accelerator-user-guide.pdf#page=26)
  describes source/destination faults with block-on-fault disabled, reporting
  progress and software continuation. Intel also supports other fault policies;
  DSA-like here denotes only the selected pattern, not universal DSA behavior.
- **[2] NVIDIA RISC-V Summit 2024 presentation, pages 4 and 10-11.**
  [GSP architecture](https://static.sched.com/hosted_files/riscvsummit2024/fe/Keynote_2024.pdf#page=11)
  shows RV64, Resource Manager and guest-associated vGPU runtime partitions.
  Context-switch is listed as an embedded RISC-V use, not specifically as a GSP
  implementation of all CE fault checkpoints. The slides do not establish a
  combined VF copy/reset FIFO or per-copy firmware interpretation.
- **[3] NVIDIA UVM, pinned driver snapshot.**
  [CE/PBDMA fault handling](https://github.com/NVIDIA/open-gpu-kernel-modules/blob/61dcc93722ecb418bb5f2e00923f05b4b8051dd1/kernel-open/nvidia-uvm/uvm_gpu_non_replayable_faults.c)
  describes fault-and-switch, page repair and rescheduling the original channel.
  clear_faulted_method_on_gpu submits through a UVM MEMOPS channel targeting the
  faulted user channel; other configurations use direct register clearing.
  SR-IOV uses a software method when privileged registers are unavailable.
  This is not evidence of firmware reconstructing a residual copy, nor of recovery
  submitted through the original copy channel.
- **[4] NVIDIA Turing command-front-end manuals, pinned documentation snapshot.**
  [PBDMA](https://github.com/NVIDIA/open-gpu-doc/blob/9fdf5c4062007929d9f4e6cbad9c9771fe61b880/manuals/turing/tu104/dev_pbdma.ref.txt)
  fetches/parses methods and dispatches them; it is not the payload Copy Engine.
  [Pushbuffer/subchannels](https://github.com/NVIDIA/open-gpu-doc/blob/9fdf5c4062007929d9f4e6cbad9c9771fe61b880/manuals/turing/tu104/dev_ram.ref.txt)
  expose common encodings with runlist-dependent engine mappings and software
  methods. They do not prove the complete current GSP/VF submission protocol.
- **[5] etha_model, pinned model snapshot.**
  [IPsec layout descriptors](https://github.com/shady831213/etha_model/blob/5270f00874a1ad390fbbce6f5395be24a52d0ec9/src/etha_ipsec/desc/req.rs#L9-L59)
  give source/destination their own field offsets. The
  [example](https://github.com/shady831213/etha_model/blob/5270f00874a1ad390fbbce6f5395be24a52d0ec9/examples/etha_ipsec/example.c#L63-L95)
  locates packet fields across a logical buffer assembled from separate blocks.
  This motivates normal range-selection use, not a page-fault recovery protocol.
- **[6] Linux device page requests and SVA.**
  [Page-request data](https://github.com/torvalds/linux/blob/v6.12/include/linux/iommu.h#L42-L74)
  includes address-space identity, address and requested permissions.
  [SVA mapping coherence](https://docs.kernel.org/arch/x86/sva.html)
  describes translation invalidation and obtaining usable translations after repair.
  These support the information/ordering boundary, not adoption of PCIe PRI
  transactions or page-request grouping into this DMA model.

Unresolved NVIDIA questions: the end-to-end guest interface to vGPU Runtime to CE
path; whether copy, control and recovery share a transport in a particular mode;
and the exact division of checkpoint/save/restore between fixed logic and embedded
firmware. Device-retained recoverability is supported by the inspected paths;
pure-RTL-only preservation and firmware-owned residual resubmission are not proven.
These gaps neither establish nor disprove the deferred PDMA alternative and do
not block the DSA-like design discussion.
