# PF-managed Function channels

## Current checkpoint

The managed platform supports an instance-configured number of Functions sharing
one firmware program and one DMA execution pipeline. A Function is a trusted
platform ingress identity, not a modeled CPU, request ID, PASID or PCIe BDF.
The ordinary direct-DMA assembly remains independent of firmware and Functions.

## Ownership boundaries

- `contract` defines `FunctionId`, the owner register, one control-register
  block, explicit request targets and typed invalid-target/ownership responses. DMA commands are unchanged.
- `hw-components::ChannelWindows` owns the channel permission registers. Firmware
  programs their owner and open bit; `for_function` enforces the requester identity
  on each complete transaction. Discovery is public. Administration is never
  granted by a channel window, and a foreign Function cannot read its readiness.
- `hw-components::ControlChannel` owns one bootstrap/doorbell block. Its immutable
  manager identity is supplied at platform assembly. Only this PF may read or write
  the Host-facing block. Firmware accesses the opposite view. No ring memory lives
  in hardware.
- Firmware validates each PF-supplied target Function against the configured
  namespace, then owns allocation, owner programming, reset and release through MMIO.
  A request target never changes the actual requester's work-MMIO identity.
- DMA reports channel-specific pulses and a separate device-wide source. It
  neither knows Function identity nor stores software handlers.
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

`ControlRegisters` has a 20-byte extent and eight-byte alignment, at a
platform-selected base. There is no Function control bank or exported control stride.
All Host ports may know its address; hardware accepts only the configured PF.
Firmware's Function count describes valid targets, not control endpoints.

A request is 24 bytes: ID at 0, opcode at 8, target Function at 12, channel at 16,
then reserved alignment padding. Responses remain 16 bytes and correlate by ID.
Out-of-range Function targets return `InvalidFunction` before resource side effects.

## Scope and evidence

This is management/MMIO/interrupt isolation, **not DMA address-space isolation**.
The memory backend is still shared. Trusted software must keep other Functions'
ring memory intact; DMA and control-memory accesses carry no Function/PASID tags.
Malformed ring or bus failures remain subsystem execution errors, not independently
recoverable Function failures. PCIe integration, PCIe PF/VF provisioning, Function reset,
FLR, PASID/context tables, event rings and hostile-memory isolation remain future work.

`firmware_managed/cases/functions.rs` exercises two real IRQ-driven copies and
reset during a partial transfer while peer DMA completes. Runtime boundary tests
exercise invalid target/ownership rejection, VF management denial, MMIO isolation,
reuse revocation, dormant VFs managed by PF, response backpressure,
and a real DMA notification delivered after reset/regrant. Hardware tests cover
VF bootstrap/doorbell rejection, independent notification directions and open-binding interlocks.

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
