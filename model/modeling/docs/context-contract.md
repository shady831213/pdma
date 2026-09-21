# Context contract for the model and future RTL

Recorded: 2026-09-21.
Status: agreed design, not implemented behavior or a released register ABI.
This document is specification input for the next modeling slice and subsequent
RTL. It does not authorize implementation automatically. Current behavior remains
described by [Function isolation](function-isolation.md),
[control channels](control-channel.md), and [reset](reset.md).

MUST and MUST NOT below describe the agreed future contract. Software obligations
are part of the hardware/software interface, not implementation notes. Preserve
their identifiers when deriving the RTL specification and verification cases.
Behavior outside these preconditions has no successful-cleanup guarantee; it MUST
NOT be described as supported hostile-software isolation.

## Address-space selection and ownership

| Access | Selector | Configuration owner |
| --- | --- | --- |
| Control bootstrap structure, control rings and their memory-resident cursors | Independent control access configuration | Management PF through BAR/MMIO before enable |
| DMA command and response rings | One channel-level `ring_context` | Authorized channel software |
| Source and destination SG tables | One descriptor-level `sg_context` | Command submitter |
| Source payload | Descriptor-level `src_context` | Command submitter |
| Destination payload | Descriptor-level `dst_context` | Command submitter |

There is no command-level default payload context. Source and destination may
select the same or different contexts. SG table bytes use `sg_context`; addresses
inside their entries use the corresponding payload context. When neither endpoint
uses SG, `sg_context` is ignored and is not a task reference.

A context is a reusable access binding, not a task allocation. Many commands may
reference one entry. Its logical contents include validity, owner Function, access
target (Host or Device), and the address-space attributes applicable to that target.
Host access can select Default or a PASID; Default is not PASID zero. Device access
must not be forced to carry a meaningless Host PASID. This selects one target per
context; it is not an AMD GPUVM page table that mixes targets page by page.
Further attributes require defined semantics, not speculative fields.

`hw-components` owns the context registers and access routing. It derives the
requesting Function from the channel's controlled ownership binding and checks
the selected entry's ownership. A descriptor cannot select another Function by
guessing a context ID. Resolved target/identity attributes accompany each request
through the fabric to its destination adapter. The DMA core carries selectors and
task state; it does not implement PCIe identity or select memory backing.

Firmware allocates, configures and releases table entries in response to PF
management requests. The Host OS/platform establishes actual memory mappings and
PASIDs. Runtime supplies backing and execution, not context allocation, lookup
policy, snapshots or abort traversal. Preserve the ordinary unit-sideband assembly
without requiring firmware or context tables.

The control access configuration is separate from the table, not reserved slot 0.
Its Function identity is the platform-selected management PF. PF directly programs
Default/PASID selection and bootstrap address, then enables the control channel.
Hardware freezes these settings on enable. Firmware's Host-memory access port uses
this configuration without first creating a table entry through the control ring.
Data-channel/context maintenance does not revoke the control path.

## Software obligations

| ID | Responsible party | Required behavior |
| --- | --- | --- |
| SW-01 | PF and platform/OS | Establish the intended accessible mappings before publishing a binding. Initialize control queue memory and control access configuration before enable. Do not reconfigure the enabled control access window. |
| SW-02 | Submitters and PF coordinator | Before issuing Drain/Abort, stop all submitters from publishing new commands referencing the target context, on every relevant channel. Keep this exclusion until successful completion; on failure retain it through recovery. Commands using only other contexts may continue. |
| SW-03 | Submitters and platform transport | Make all prior descriptor stores and producer publications visible to hardware before the maintenance request can take effect. PF/VF or CPU-to-CPU coordination must establish this ordering; merely sending a later control message is not sufficient by itself. |
| SW-04 | Submitters | Keep published descriptor contents stable until consumed; retain SG lists, payload storage and mappings until ordinary task completion or the successful maintenance boundary that ends their use. Abort does not interrupt an already-started command. |
| SW-05 | Channel users | Keep ring storage and its context binding valid while hardware can still use it. Disabling submission alone does not prove that accepted work and response writes have ended. |
| SW-06 | Host software | Continue normal response consumption while waiting for maintenance. Software need not count responses to prove Drain/Abort completion and must not wait for a per-command response from an Abort-discarded command. |
| SW-07 | Host software | Treat successful Abort as collective termination of its old target tasks, not proof that none executed. Handle normal responses already published but not yet consumed; do not reuse task IDs in a way that confuses these with new tasks. Pending interrupt delivery is not retracted. |
| SW-08 | PF and channel users | Do not reset/release/rebind affected channels or change their ring geometry/context while maintenance is pending. Hardware interlocks enforce the corresponding configuration restrictions. Failure recovery must still permit the required reset. |
| SW-09 | PF and submitters | When the intention is destruction, maintain the no-new-submission interval through Destroy success. Do not resume after an intermediate Drain/Abort and then rely on that old completion to justify destruction. |
| SW-10 | PF and platform/OS | Do not free or rebind a context, recycle its slot, or tear down dependent mappings based on a failed request, timeout or abandoned software waiter. Preserve them until a successful recovery establishes the required boundary. |
| SW-11 | Platform/OS | DestroyContext releases a PDMA binding, not necessarily the PASID or entire address space. Account for aliases, control/ring bindings and other devices before unmapping or recycling shared resources. |
| SW-12 | Trusted PF/driver | Distribute only authorized selectors and never submit a stale destroyed ID. Function ownership checks do not establish per-process isolation inside one Function. Actual memory mapping permissions remain authoritative. |

Host/VF coordination with the PF remains a software/platform responsibility; this
contract does not introduce a VF mailbox. Violating SW-02 or SW-03 invalidates the
snapshot proof. Hardware cannot infer unpublished future commands from a table
entry or from its current in-flight task count.

## Operations and completion semantics

These are semantic operation names; opcodes and wire layouts are not assigned.

| Operation | Effect | Successful result |
| --- | --- | --- |
| CreateContext | Establish a complete, authorized binding before making it usable | Return a reusable selector; table exhaustion is not per-command exhaustion |
| DrainContext | Let old tasks referencing the context finish normally | Old target tasks have retired; original binding remains usable |
| AbortContext | Discard old target tasks not yet started; let started tasks reach their ordinary terminal outcome | Old target tasks have retired or been discarded; original binding remains usable |
| DestroyContext | Release a binding only after its old references and accepted accesses have exited | Selector no longer usable; slot eligible for reallocation |

Successful Drain and Abort do not change the binding and need no ResumeContext.
Destroy is not an unrestricted write to `valid`. The chosen management encoding
must preserve SW-09 whether cleanup is part of Destroy or a prerequisite sequence.
An operation that encounters a channel fault follows the failure path below;
"successful result" must not be inferred merely from receiving a reply.

HW-01: A context still bound to a ring that hardware may access MUST NOT be
revoked or destroyed. Report a resource-busy outcome rather than disabling its
translation. Safely stop/reset and detach the ring first. Whole-context maintenance
cannot silently exempt ring traffic while claiming the context is quiescent.

HW-02: Define a single execution-acceptance point ordered against an Abort request.
A target command before that point may be discarded; a command already past it
continues under its original immutable bindings. A prefetched descriptor is not
automatically a started command. The implementation/spec must identify the exact
acceptance point, not infer it from first payload write or consumer movement.

HW-03: Abort is command-granular, not per-memory-access cancellation. Discarded
commands consume their command-ring entry but perform no SG/payload accesses and
need not publish a response or completion interrupt. Their discard path must not
require response-ring space. Other commands retain ordinary execution/admission
rules. Abort must not simply invalidate a table entry that a started command is
still using. Ordinary faults and channel/device reset retain their own semantics.
This is not transactional rollback: writes already performed are not undone.

HW-04: Normal responses from started tasks are published through the retained ring
context. Real errors keep the ordinary report/halt/reset policy; no new per-task
Aborted response or non-halting error policy is required merely for discarded work.
Do not cancel tasks whose only use of the target ID is an ignored SG selector.

## Hardware snapshot and retirement proof

HW-05: On maintenance acceptance, hardware fixes a set of channels that covers
every channel capable of containing old references to the target. It snapshots
their published producers internally; Host neither supplies nor tracks snapshots.
Capturing all channels owned by the target Function is a conservative option.
No per-context list of unread descriptors is required. Sequential per-channel
snapshots are sufficient under SW-02/SW-03; all snapshots need not occur in one cycle.

HW-06: For each captured channel, retain its queue incarnation and a sticky fact
that its consumer has reached/passed the captured producer. Use the existing
wrapping-count ring rules, not numerical `consumer >= producer`. Later producer
movement must not extend the captured frontier, and later counter wrap must not
undo a reached frontier. An empty captured queue is already through its frontier.
Interlock conflicting configuration/lifecycle changes while the operation is pending.

HW-07: Consumer progress proves descriptor consumption, not execution completion.
Register accepted task ownership without an observable gap when advancing consumer,
and keep it until the complete command pipeline retires. Preserve the existing
`RunCommand` terminal boundary; do not release ownership merely on payload completion
or response publication. Deduplicate multiple uses of the same context within a
task when using reference counts. The current single executing command does not
require a speculative multi-engine ledger.

Successful maintenance requires all of:

1. Every captured channel has passed its producer frontier.
2. Every accepted task referencing the target has reached its terminal outcome.
3. All external accesses belonging to those tasks have reached their qualified
   completion point; none can later issue further accesses from the old task.

Proof: SW-02/SW-03 place every old target descriptor either before a captured
frontier or in already-accepted work. Condition 1 removes the unread set;
conditions 2 and 3 remove the accepted set. There is no remaining source of old
target work. Other contexts may continue to submit and execute, although old
unrelated work before a frontier can delay completion. This is not a latency or
preemption guarantee.

The synchronous model completes each memory call within a step and preserves
accepted command state across steps. RTL must also drain every affected outstanding
transaction, including committed reads/writes and required responses. Neither a
counter reaching zero too early nor receipt of only the most recent bus response
can substitute for this guarantee. Destination-port completion/visibility rules
must be specified during bus integration; model-local completion is not proof of
arbitrary PCIe transport completion.

## Failure and recovery are not successful drain

HW-08: If a captured channel faults or otherwise cannot satisfy the frontier and
retirement conditions, report maintenance failure with enough channel information
for recovery. Firmware must return a control-protocol failure response and continue
servicing later requests, not terminate its service or wait indefinitely for a
reset request queued behind the blocked operation. Common response backpressure
and transport faults retain their general handling, not a new context-only protocol.

HW-09: End or supersede the failed maintenance operation in a defined way, allow
the necessary channel reset, and preserve bindings/storage until recovery proves
cleanup. Failure must neither leak a maintenance lock that prevents reset nor make
the table slot free. Software retains SW-02/SW-09 exclusion while recovering.
Partial progress on some channels is not whole-operation success. An unexpected
queue reset or incarnation change must not make a fresh consumer satisfy an old
frontier. Reset may discard unpublished responses under the existing reset contract.

A software timeout or cancellation does not cancel hardware work or establish
cleanup. A genuinely unresponsive execution/bus path cannot be made safe by a
synthetic successful control response. Error/cancellation recovery details must
preserve this distinction when the external transport is implemented.

## RTL specification and verification handoff

The final specification MUST retain: the software preconditions above; hardware
acceptance and completion points; register ownership/interlocks; ordering and
visibility requirements; failure versus successful-cleanup meaning; reset scopes;
and the permitted effects of discarded or partially executed tasks. Model scenarios
must test these guarantees without concealing software obligations in the harness.

Future verification obligations, not claims of existing test coverage:

| Scenario | Required observable | Contract |
| --- | --- | --- |
| Distinct ring, SG, source and destination spaces with identical addresses | Every access reaches the intended authorized space | Selection table, SW-01/12 |
| Control bootstrap before any data context exists | Management starts without context-table allocation | SW-01, independent control path |
| Mixed-context queued work with later unrelated submissions | Only fixed old frontiers are awaited; unrelated tasks are not discarded | SW-02/03, HW-05/06 |
| Abort before/after execution acceptance | Before: no payload/SG or task response; after: ordinary completion | HW-02/03/04 |
| Consumer reaches frontier while a target command is still active | No early maintenance success | HW-07 |
| Multiple channels and wrapping pointers, including an empty queue | All old work covered; reached frontiers stay reached | HW-05/06 |
| Active ring context is a maintenance target | No loss of live ring access or premature resource release | SW-05, HW-01 |
| Channel faults during maintenance | Control failure returns; reset can proceed; binding is not reused | SW-10, HW-08/09 |
| Standalone Drain/Abort succeeds, then new target work is submitted | Same binding works without ResumeContext | Operation table |
| Destroy after cleanup, failed cleanup, aliases and stale software waiters | Only valid successful release authorizes slot reuse; no implicit global PASID unbind | SW-09/10/11/12 |

Before RTL/ABI freeze, resolve selector widths and table capacity, target-specific
address attributes and Device-memory permissions, exact register/command layouts,
execution-acceptance placement, maintenance error encoding, and management reset
recovery. Generation-tagged selectors are not required by the agreed trusted-software
protocol when all old references exit before reuse; adopting them needs separate
semantics and does not replace draining. These open implementation details must
not weaken the agreed software obligations or hardware completion guarantees.

## Supporting references and limits of analogy

- [NVMe 2.0d, Delete I/O Submission Queue, section 5.7](https://nvmexpress.org/wp-content/uploads/NVM-Express-Base-Specification-2.0d-2024.01.11-Ratified.pdf): successful deletion implicitly terminates commands without individual completion entries. This supports collective completion, not identical context-scoped abort hardware.
- [Virtio 1.2, section 2.6.1](https://docs.oasis-open.org/virtio/virtio/v1.2/virtio-v1.2.html): queue resources may be released after confirmed reset. Protecting the ring's context binding follows that lifetime invariant; Virtio does not specify this PDMA table.
- [Intel DSA architecture, section 3.15](https://cdrdv2-public.intel.com/857060/341204-006-intel-data-streaming-accelerator-spec.pdf): abort may complete or abandon tasks and does not guarantee interruption of started operations. It does not establish our exact ring-snapshot implementation.
- [CUDA event semantics](https://docs.nvidia.com/cuda/archive/12.6.0/cuda-runtime-api/group__CUDART__EVENT.html): recording captures a fixed work set unaffected by later submissions. This supports a fixed completion frontier, not a context cancellation algorithm.
- [AMDGPU GPUVM](https://docs.kernel.org/gpu/amdgpu/driver-core.html#amdgpu-virtual-memory): one GPUVM can mix system and VRAM pages. Our per-context target selection is deliberately a different granularity.
