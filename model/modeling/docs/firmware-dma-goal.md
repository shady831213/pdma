# Next goal: firmware-managed DMA

Recorded: 2026-09-15.
Status: broader architectural direction; the first architecture MVP is implemented,
while virtualization, context selection and Function lifecycle features remain future work.
This document records decisions and open questions; it is not an execution plan
or an instruction to start implementation automatically.

First milestone: the [firmware-managed DMA architecture MVP](firmware-dma-mvp.md).
The subsequent [control channel](control-channel.md) implements independent
command/response rings for single-channel acquisition/release, including local
reset completion. Function-aware ownership and external drain remain future work.
Validate the ownership and runtime assembly boundaries before implementing the
broader virtualization and address-space features below.

## Objective

Use firmware plus the existing shared DMA hardware as the primary architecture
for preparing SR-IOV, PASID and function-level reset support. Keep policy and
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
- Commands select an address-space context using `context_id`. This is a reusable
  context selector, not the software task identifier. Preserve task ID as a
  submission/response correlation value, not a hardware lookup-table key.
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
`Device` internals. Firmware does not replace the hardware DMA data path.

### Code and assembly boundaries

The agreed target is separate `dma`, `hw-components` and `firmware` crates, with
shared soft/hard definitions in `contract` and Host software interfaces in `host`.
`hw-components` is a collection of concrete hardware objects, not a generic bus
fabric or a firmware policy controller. Each object owns its own registers/state.
The core now lives in `crates/dma`; the first access window and firmware program
live in `crates/hw-components` and `crates/firmware` respectively.

Firmware is an async program. Host and firmware reuse the existing software CPU
implementation with separate CPU instances, CPU-local state and capability views.
Each domain uses an interrupt runtime only when it has interrupt inputs; the MVP
firmware currently needs none. Sharing an async executor does not mean sharing a
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

Firmware coordinates these facts; writing a completion register cannot fabricate
memory retirement. A stalled firmware must not reopen a closed scope. Management
firmware and its access path must survive the Function reset they coordinate;
whole-device reset needs a path that does not require that firmware to run.
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

- Context selector width, table capacity, namespace and authorized programming
  interface; exact descriptor and register layouts remain undecided.
- Whether source and destination select independent contexts, and which contexts
  command rings, response rings and SG-list reads use. A command's context cannot
  determine the access needed to fetch that command itself.
- Exact management access permissions, stop/rebind interlocks and ownership of
  local-completion, external-drain and release observations.
- Asynchronous management events, management reset domain and recovery when
  firmware is unavailable. The current polled control command/response protocol
  does not include an event ring or Function lifecycle protocol.
- Scheduling policy when an accepted task waits on memory. Retaining one shared
  engine does not automatically provide cross-Function progress while it waits;
  do not silently introduce per-channel suspended execution or preemption.
- Which features are instance configuration and which have software enable bits.

## Future acceptance focus

Retain ordinary copy, SG, fault, reset and interrupt scenarios. Add explicit
hardware/firmware/host/harness separation and test unauthorized resource access,
context isolation, queued-context reuse, delayed memory completion during reset,
local completion before external release, stale notifications and unaffected
channel behavior. These are future requirements, not claims of current coverage.

## Reference boundary

Reference repository: `https://github.com/shady831213/pcie_modeling`.
Its `docs/architecture/pcie-endpoint-architecture.md` APP-CONTROL section and
`crates/pcie-app/src/control/` distinguish request acceptance, local completion
and manager release. The architecture document records an ongoing migration:
recheck actual interfaces when integrating. Do not restore its legacy centralized
per-work admission/token tables into PDMA. PCIe capabilities, ATS/PRI, MSI-X and
DWC-specific handshake encoding remain outside this preparation goal.
