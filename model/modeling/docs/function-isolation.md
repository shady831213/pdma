# PF-managed Function channels

## Current checkpoint

The managed platform supports an instance-configured number of Functions sharing
one firmware program and one DMA execution pipeline. A Function is a trusted
platform ingress identity, not a modeled CPU, request ID, PASID or PCIe BDF.
The ordinary direct-DMA assembly remains independent of firmware and Functions.

## Ownership boundaries

- `contract` defines `FunctionId`, the owner register, one control-register
  block, explicit request targets and typed invalid-target/ownership responses.
  Context-enabled DMA commands select bindings, never a Function identity.
- Each `hw-components::windows::ChannelWindow` owns its permission registers. Firmware
  programs its owner and open bit; `read_for`/`write_for` enforce the live owner
  before readiness, open-state and transaction-bound checks. `ChannelWindows`
  holds the windows and routes addresses; `for_function` binds the requester and
  delegates to the selected window. Discovery is public. Administration is never
  granted by a channel window, and a foreign Function cannot read its readiness.
  Direct window/bank `read`/`write` are caller-authorized access paths; Function-bound
  Host ingress uses `for_function`, without caching owner or permission decisions.
- `hw-components::control::ControlChannel` owns one bootstrap/doorbell block. Its immutable
  manager identity is supplied at platform assembly. Only this PF may read or write
  the Host-facing block. Firmware accesses the opposite view. No ring memory lives
  in hardware.
- Firmware validates each PF-supplied target Function against the configured
  namespace, then owns allocation, owner programming, reset and release through MMIO.
  A request target never changes the actual requester's work-MMIO identity.
- DMA reports channel-specific pulses and a separate device-wide source. It
  neither knows Function identity nor stores software handlers.
- Generic `MemoryAccess<S = ()>` carries explicit metadata. DMA emits channel/context
  selectors. Without a context table, `hw-components::windows::ChannelMemoryAccess` borrows
  the authoritative `ChannelWindows` bank and forwards FunctionId. The optional
  context table additionally validates binding ownership and resolves Default/PASID
  attributes into `MemoryIdentity` for the runtime's memory-space adapter.
  MMIO, interrupt routing and DMA memory use that same ownership bank. Ordinary
  unit-sideband backends keep their existing read/write call signatures.
- The scenario platform supplies private Function-bound Host bus capabilities,
  independent software CPUs/interrupt runtimes and pending interrupt destinations.
  It wires hardware events to those destinations, without allocating channels.

PF software owns the only `Management` client, ring buffers, request-ID namespace
and response consumption. VF software uses only granted DMA channels. Firmware
attaches one endpoint and serially awaits accepted lifecycle operations. A reset
can delay later management requests, and a full PF response ring stops admission;
neither condition stops independently driven peer DMA. This is not a preemptive
firmware scheduler or FLR coordinator.

## Lifecycle and notification ordering

Acquisition programs the owner while the channel window is closed, then opens it.
Reset/release first verifies the specified target owns the selected channel; a mismatched
target returns `PermissionDenied` without touching its window or DMA state.

Reset closes access and awaits DMA's real reset handshake before reopening the
same owner's unconfigured channel. Release closes access and awaits the same
handshake before freeing the allocation. Reuse then programs the new owner and
opens its window. PF must coordinate with VF software to retain old memory until reset completes; reset
does not undo completed writes. Owner-register writes while open are rejected.
Closing a window alone is not evidence that DMA has drained.

The platform transfers each DMA pulse to the currently bound destination at the
hardware interaction boundary, before any later binding update. The pending
interrupt belongs to that destination until claimed, even after reset/regrant.
The new owner never receives an old owner's pending message. Old handlers must
tolerate a notification whose former channel is no longer accessible. The managed
platform wires the separate device-wide source to the management destination;
device-fatal recovery is not extended by this slice.

## Register geometry

`ChannelWindowBlock` occupies 12 bytes: open at 0, owner at 4, readiness at 8.
Owner is a firmware-only 32-bit Function identity; reset value zero supports the
single-Host configuration. Both static header views retain the complete layout.

`ControlRegisters` has a 36-byte extent and eight-byte alignment, at a
platform-selected base. There is no Function control bank or exported control stride.
All Host ports may know its address; hardware accepts only the configured PF.
Firmware's Function count describes valid targets, not control endpoints.
The independent event doorbell and firmware-owned event-error report belong to
this same PF management aperture; they grant no VF administrative access.

A request is 32 bytes: ID at 0, opcode at 8, target Function at 12, channel at 16,
context at 20, and creation attributes at 24. Responses are 24 bytes: ID at 0,
status at 8, channel at 12, context at 16, then reserved alignment padding.
Responses correlate by ID. These layouts come from the current
[contract schemas](../crates/contract/src/control.rs); the
[generated header guide](../include/README.md) describes the complete wire views.
Out-of-range Function targets return `InvalidFunction` before resource side effects.

## Scope and evidence

The managed assembly can now supply independent DMA address spaces per Function.
`Managed::with_function_memory` binds supplied capabilities by Function ID; the
shared-memory assembly remains available through `with_functions`. Separate spaces
may map identical addresses to different bytes. This is injected-backend isolation,
not PCIe/IOMMU/PASID translation or protection against a malicious integration.
Malformed ring or bus failures remain subsystem execution errors, not independently
recoverable Function failures. PCIe integration, PCIe PF/VF provisioning, Function reset,
FLR, event rings and hostile-memory isolation remain future work. Optional PASID
bindings and context maintenance are specified in [the context contract](context-contract.md).

`firmware_managed/cases/functions.rs` exercises two real IRQ-driven copies and
reset during a partial transfer while peer DMA completes. Runtime boundary tests
exercise invalid target/ownership rejection, VF management denial, MMIO isolation,
reuse revocation, dormant VFs managed by PF, response backpressure,
and a real DMA notification delivered after reset/regrant. Hardware tests cover
VF bootstrap/doorbell rejection, independent notification directions and open-binding interlocks.

## DMA memory boundary

All command fetches, source/destination SG-list reads, payload reads/writes and
response publication use the selected channel's memory port. Neither descriptors
nor VF software select a Function ID for these accesses. A missing owner capability
fails as an unmapped access, without falling back to PF or another Function; existing
command/response fatal and list/payload response-error semantics remain unchanged.

The platform owns backing stores and gives each software program its corresponding
allocator and memory capability. Firmware's control-ring memory is the PF's space,
not a union of VF spaces. Sharing a backend deliberately shares its address space;
constructors do not infer isolation or clone the underlying bytes.

`ChannelWindows::memory` creates a borrowed hardware view over a supplied
`MemoryAccess<FunctionId>` port. It resolves one owner per synchronous access and
forwards the address, alignment, length/buffer and existing downstream error unchanged.
It holds no backing, allocation, synchronization or cached owner state. Runtime
retains `FunctionSpaces` and the shared window lock; its `RoutedMemory` only locks
and delegates to this hardware view. Lock order is hardware, windows, then backing;
downstream backends must not reenter hardware/window locks. No lock spans an await.

Every valid channel has an owner register, initially zero; release does not clear it.
With correct firmware and platform wiring, admitted DMA requests always have a valid
owner and supplied Function capability. Invalid channel IDs or missing capabilities
remain model/integration errors reported through the existing memory error, not a
new hardware lookup-failure protocol or a fallback to Function zero.

DMA binds its sideband to a borrowed memory view for each synchronous interaction.
Closing Host MMIO does not revoke old DMA access or change its identity. Firmware
must keep the owner unchanged until channel reset completes, then may reassign it.
Existing firmware release already follows that order; live owner rebinding before
drain is unsupported. The DMA generation guard and terminal command retirement
prevent retained old work from accessing the new owner's memory after reset.
If a future external bus queues requests or permits outstanding transactions, each
accepted request must retain its resolved FunctionId and reset completion must wait
for all affected transactions to drain. This synchronous model adds no such queue.

The managed Function cases cover identical-address IRQ-driven copies and reset
during a transfer with independent peer memory. Managed runtime memory boundary tests
cover command/list/payload/response routing, unmapped foreign addresses and reset
before regrant, absent bindings and closed-window access.
`hw-components/tests/windows/memory.rs` directly verifies identity translation,
raw/typed access, closed-window retention, reassignment and error propagation.
The generic memory API adds explicit sideband raw and typed methods; ordinary methods
delegate with unit sideband. Non-unit metadata cannot be omitted, even if its type
implements Default. Descriptors and register layouts are unchanged.

Scenario PF/VF grant and reset coordination uses test signals, not a new modeled
PF-to-VF protocol.

## Agreed VF pre-FLR integration contract

Recorded: 2026-09-20. This decision supersedes the earlier plan for an independent
FLR ingress into firmware. It requires no current code enhancement: PCIe FLR and
its integration handshake are not implemented or tested by this checkpoint.

The supported future sequence relies on one trusted PF:

1. PF serializes the target VF's lifecycle, stops its software ring/ISR use and
   prevents further acquisitions for that VF, including already queued requests.
2. PF submits existing `ReleaseChannel` requests for every channel owned by the VF
   and collects every successful response. Firmware closes each window, awaits
   actual DMA reset completion and frees the allocation without reopening access.
   Submission, a timeout, cancellation or a failed response is not completion.
3. Only then does PF initiate PCIe VF FLR. PF keeps new allocations for that VF
   excluded until the PCIe reset has actually completed.
4. After FLR, PF may acquire channels again and software configures fresh rings.
   No old allocation or task is restored automatically.

The PF control ring pair, its doorbells and firmware remain outside this VF's
reset scope. They continue to manage other Functions; releasing VF DMA channels
does not release the shared PF management channel. `ResetChannel` alone is not
the preparation operation because it retains allocation and reopens access.
No additional Function prepare/resume command is needed for current DMA resources.

Under this precondition, the PDMA application side has no remaining cleanup to do
when VF FLR arrives. A future integration endpoint can acknowledge each valid reset
request immediately, preserving its request/Function association, without querying
firmware ownership, resetting channels again or notifying firmware. At the signal
level this application-ready condition can be constant true. Whether an actual IP
done pin may be tied high or must be request-gated depends on that IP's contract;
this decision does not certify either wiring for a particular controller.

Application ACK is not PCIe FLR completion. The PCIe integration layer owns its
required transaction drain, any future ATS/PRI obligations, controller done/release
handshake and request association. A release response establishes the current
model's local DMA cleanup, not a modeled PCIe drain. Already platform-pending DMA
notifications are not retracted by channel reset. Future external-memory/context
support must establish its additional cleanup obligations before reusing this
immediate-ACK policy.

Direct FLR without successful pre-FLR release is outside the supported sequence.
There is no resource-precondition checker, allocation interlock during FLR or
automatic cleanup fallback in this design; the software sequencing above is a
platform requirement, not an enforced hardware guarantee. Do not claim that direct
FLR is detected or safely recovered. No new firmware ingress, ISR, MMIO handshake,
ownership table or DMA FLR state machine is required by this decision.

PF FLR and whole-subsystem reset need separately defined reset scopes, including
the shared control channel and firmware lifetime. They are not covered by this VF
contract. The ordinary direct-DMA assembly remains unchanged.
