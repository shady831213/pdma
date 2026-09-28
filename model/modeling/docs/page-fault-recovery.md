# Pageable-copy recovery: DSA-like direction

Recorded: 2026-09-28.
Status: discussion checkpoint and selected next design focus; not implemented,
not a frozen ABI, and not an instruction to start implementation automatically.
Resume here after the holiday. The [roadmap](firmware-dma-goal.md) sets priority;
the [system specification](spec.md) remains authoritative for current behavior.

## Two distinct copy uses

Prepared page migration uses the existing ordinary-copy behavior with the
[non-faulting migration preconditions](context-contract.md#non-faulting-page-migration-copy).
An original CPU/GPU fault may trigger migration, but its servicing copy must have
accessible payload, queue and SG storage. This does not implement an OS pager,
page-table updates or MMU invalidation.

The next design concerns copying pageable virtual addresses whose own accesses
can fault. It adopts the partial-completion and software-continuation pattern,
not Intel's register layout, submission portal or complete feature set. Intel's
non-blocking source/destination fault handling provides the relevant precedent [1].

## Selected direction

1. Host/VF software submits through its existing authorized DMA command ring.
2. On a specifically classified recoverable access fault, DMA stops that hardware
   command and publishes a response describing the fault and reliable progress.
3. After publication and normal retirement, that hardware command is terminal.
   DMA retains no suspended task waiting for resume and can service later work,
   subject to ordinary admission and response-space constraints.
4. Host software arranges page/mapping repair through its OS/platform, constructs
   the remaining copy and submits it as a new command. Repeated faults repeat this
   process; policy may instead fail or abandon the logical operation safely.
5. Software correlates these hardware attempts with the original logical task and
   decides when that logical task is complete. Partial completion is not success.

This is a target behavior, not today's error policy. Currently Response reports
bytes_completed but has no dedicated recoverable-page-fault result or fault-address
record. Unsuccessful responses stop their channel until reset. Ring-access faults
use the existing fatal path. Do not reinterpret current ReadFault, WriteFault or
ListReadFault as resumable paging failures without defining the new distinction.

## Responsibility boundaries

| Owner | Responsibility in the proposed slice |
| --- | --- |
| DMA hardware | Stop at a defined side-effect boundary, report authoritative progress and fault details, publish the terminal response and retire the command. No page repair or implicit retry. |
| Contract | Encode the agreed recoverable outcome and diagnostic/progress information after the semantics are fixed. No speculative fields yet. |
| hw-components | Preserve channel/context authorization and access identity routing; do not move binding lookup or Function policy into DMA. |
| Host software | Repair through platform services, build continuation commands, maintain logical-task identity/dependencies and obey context lifetimes. VF need not forward every fault through PF/firmware. |
| Firmware | Keep existing resource and lifecycle management. No per-copy continuation state, new VF control queue or proxy submission requirement. |
| Runtime/platform | Supply memory capabilities and explicitly classified access outcomes; OS/MMU owns actual mapping repair. No PDMA continuation policy in runtime or hidden harness behavior. |

Backend allocation failures, unmapped model storage, permission denial, invalid
context binding and recoverable translation faults are not automatically the same
condition. Define the injected access-result contract without implementing an
IOMMU or lowering the model to bus cycles and outstanding transactions.
Direct, context-free operation must remain usable without firmware or a pager.

## Decisions needed before implementation

- **Committed progress.** Define whether progress is a contiguous logical prefix,
  the source-read versus destination-write boundary, and what a failed access can
  already have changed. A byte count must identify what can safely be omitted or
  retried. No rollback is implied. Existing overlap policy stays software-owned;
  do not assume replay of the whole original copy is safe or add hardware overlap
  rejection. Software must determine whether continuation preserves its intent.
- **Diagnostic information.** Agree fault classification, address and access role
  (source, destination or metadata), plus enough identity to repair the correct
  context. Widths, status values and response layout remain open.
- **SG continuation.** Relate progress to both independently segmented endpoints.
  Specify how software reconstructs segment index/offset and remaining totals,
  including SG reads during validation versus copying and faults before any write.
  Do not treat descriptor-fetch, SG-list and payload failures identically by default.
- **Reliable reporting.** Decide which metadata accesses can be recoverable.
  Command/response storage must not silently inherit payload recovery semantics;
  a failed response write cannot report itself through that same response. Keep
  response-full backpressure and reporting/fatal failures explicit. Recoverable
  responses use the response path, not an automatic PF fatal event. Their IRQ/cause
  behavior must be specified rather than inherited from today's all-errors halt.
- **Ordering.** Subsequent work may run before a continuation is submitted. A new
  command does not retain the old ring position. Software must prevent dependent
  consumers from treating a partial result as complete; decide the submission
  obligations before introducing any new fence feature.
- **Lifetime and reset.** Hardware retirement of one attempt is distinct from the
  lifetime of a software-held continuation. Define interactions with reset,
  ReleaseChannel and context Drain/Abort/Destroy. Existing submission exclusion
  applies to retries too; maintenance must not be followed by a stale continuation
  using a recycled context or channel. Timeout/caller cancellation is not cleanup.
- **Repeated faults.** Define correlation and no-progress handling. Do not add
  automatic fault merging, unbounded retry or a hidden retained hardware task.

Next session: settle these semantics first, update the normative spec, then add
failing behavioral scenarios and implement. Candidate coverage includes source
and destination faults after visible writes, independent SG boundaries, zero
progress and repeated faults, peer-channel progress, reporting failure, and reset
or context maintenance before resubmission. These are future tests, not coverage
claims. No model implementation or generated-header change is part of this note.

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

Checked: 2026-09-28. Vendor-specific facts below motivate questions, not mandatory
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

Unresolved NVIDIA questions: the end-to-end guest interface to vGPU Runtime to CE
path; whether copy, control and recovery share a transport in a particular mode;
and the exact division of checkpoint/save/restore between fixed logic and embedded
firmware. Device-retained recoverability is supported by the inspected paths;
pure-RTL-only preservation and firmware-owned residual resubmission are not proven.
These gaps neither establish nor disprove the deferred PDMA alternative and do
not block the DSA-like design discussion.
