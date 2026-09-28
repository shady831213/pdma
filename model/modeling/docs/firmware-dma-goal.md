# Firmware-managed DMA roadmap

Recorded: 2026-09-15.
Updated: 2026-09-28.
Status: the architecture MVP, Function-scoped channel management, context selection
and maintenance, and managed fatal/event recovery are implemented. Actual PCIe/IOMMU
integration and Function-level reset remain unimplemented. The next design focus
is DSA-like partial completion and Host continuation for pageable-VA copies;
synchronization and fence semantics follow.
This document records decisions and open questions; it is not an execution plan
or an instruction to start implementation automatically.

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

The next focus is the second use: copying pageable virtual addresses, where DMA's
own access can encounter a recoverable page fault. The selected direction for the
next design pass is **DSA-like partial completion followed by Host software
continuation**, retaining direct Host/VF use of DMA rings. DMA ends the affected
hardware command with precise progress and fault information; software repairs
the mapping and submits the remainder as a new command. Firmware does not proxy
each copy or retain its suspended progress.

The [page-fault recovery design note](page-fault-recovery.md) records the agreed
responsibilities, unresolved completion/SG/order semantics, and NVIDIA evidence
limits. It is not an implemented feature or a frozen ABI. Existing unsuccessful
responses still stop the channel until reset; the proposed recoverable outcome
must be distinguished from those errors before implementation.

Device-retained resume, firmware-owned VF task queues and MMU-transparent retry
remain alternatives outside this next slice. Do not restore the retired resume
API or introduce per-channel suspended tasks from the research discussion.
Keep the existing functional abstraction: no bus cycles, transaction IDs,
outstanding queues or asynchronous memory-transaction model are requested.

### Then: synchronization and fences

Define dependency waits, completion signaling, ordering scope and memory visibility
separately. Decide which operations belong to DMA commands and which belong to the
MMU/platform or controlling software. Copy completion must not silently imply cache
maintenance or TLB invalidation. No fence encoding, new opcode or register is fixed yet.

### Deferred

Full PCIe/GPU-MMU integration is not part of the first discussion. Recovery without
working firmware remains the separate low-priority TBD below. Update the normative
spec and implement focused scenarios only after the relevant semantics are agreed.

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
  queue-owning driver (configuration, submission and response consumption).
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
  submission/response correlation value. See the [context contract](context-contract.md).
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
| Host software | Use granted channels/contexts, allocate buffers, submit commands, consume responses, handle interrupts and coordinate OS address-space lifetimes. |
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
Both transports reuse the framework's pending-interrupt component. The framework's
default-on `async-runtime` feature isolates Tokio/CPU/interrupt dependencies from
the synchronous APIs used by DMA. A generic shared-context wrapper was evaluated
but not introduced: model-specific projection and IRQ extraction still belong at
the consumer, and the remaining mutex/wakeup mechanics do not justify extra hooks
or traits. A memory-only package split remains undecided, not a current requirement.

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
response consumption and permitted channel/IRQ configuration, without firmware
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
   response writes or notifications.
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
- Ordering after a terminal partial completion and later software continuation,
  as discussed in the page-fault note. Scheduling suspended tasks or synchronization
  waits remains separate future work, not an implicit addition to this slice.
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
behavior. Page-fault recovery and synchronization scenarios depend on the upcoming
semantic decisions. External transaction drain and release remain RTL/platform
obligations, not a mandate to lower the model to bus-transaction timing.

## Reference boundary

Reference repository: `https://github.com/shady831213/pcie_modeling`.
Its `docs/architecture/pcie-endpoint-architecture.md` APP-CONTROL section and
`crates/pcie-app/src/control/` distinguish request acceptance, local completion
and manager release. The architecture document records an ongoing migration:
recheck actual interfaces when integrating. Do not restore its legacy centralized
per-work admission/token tables into PDMA. PCIe capabilities, ATS/PRI, MSI-X and
DWC-specific handshake encoding remain outside this preparation goal.
