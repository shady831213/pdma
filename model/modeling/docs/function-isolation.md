# Function-scoped channel management

## Agreed next revision: PF-only management ingress

Decision recorded on 2026-09-18; implementation deferred to the next session.
The checkpoint below still implements one control channel per Function.

- Replace the per-Function control bank with one PF-to-firmware control channel
  using `ControlRegisters`. VF software does not directly submit firmware requests.
- PF software requests allocation, channel reset and release for an explicit target
  `FunctionId`; firmware retains actual allocation policy and lifecycle sequencing.
  Only the trusted PF management ingress may specify a target Function. This is
  distinct from the requester identity used to authorize ordinary VF MMIO.
- Keep channel owner registers, Function-aware work-window checks, interrupt routing
  and original-destination ownership of pending notifications.
- Remove per-Function management rings, bootstrap/doorbell banks and service scanning;
  retain separate DMA data paths and software views for each Function.
- Future hardware-originated FLR remains an independent firmware ingress. It must
  not depend on PF software submitting a command or consuming a control response.
- Preserve ordinary direct-DMA operation. This revision does not itself add PASID,
  DMA memory isolation or PCIe integration.

The remaining sections describe the implemented checkpoint, not this next revision.

## Current checkpoint

The managed platform supports an instance-configured number of Functions sharing
one firmware program and one DMA execution pipeline. A Function is a trusted
platform ingress identity, not a modeled CPU, request ID, PASID or PCIe BDF.
The ordinary direct-DMA assembly remains independent of firmware and Functions.

## Ownership boundaries

- `contract` defines `FunctionId`, the owner register, repeated control-register
  blocks and `PermissionDenied` management response. DMA commands are unchanged.
- `hw-components::ChannelWindows` owns the channel permission registers. Firmware
  programs their owner and open bit; `for_function` enforces the requester identity
  on each complete transaction. Discovery is public. Administration is never
  granted by a channel window, and a foreign Function cannot read its readiness.
- `hw-components::FunctionControls` owns independent bootstrap/doorbell blocks.
  A Host port cannot read or write another Function's block, even by guessing its
  address. Firmware can access all blocks. No ring storage lives in hardware.
- Firmware keeps the allocation policy table. The control-bank slot determines
  the requester's Function; Host cannot choose it in a request. Firmware alone
  performs allocation, owner programming, reset and release through MMIO.
- DMA reports channel-specific pulses and a separate device-wide source. It
  neither knows Function identity nor stores software handlers.
- The scenario platform supplies private Function-bound Host bus capabilities,
  independent software CPUs/interrupt runtimes and pending interrupt destinations.
  It wires hardware events to those destinations, without allocating channels.

Host programs use the same `Management` and DMA drivers as before. Each Host owns
its control-ring buffers, request-ID namespace and response consumption. A shared
firmware ISR wakes the service to examine all control rings. An uninitialized or
response-full ring does not prevent service of another ring. Firmware processes
at most one request per Function per scan and awaits each accepted lifecycle
operation serially. A reset can therefore delay other management operations,
but it does not stop independently driven peer DMA. This is not a preemptive
firmware scheduler or FLR coordinator.

## Lifecycle and notification ordering

Acquisition programs the owner while the channel window is closed, then opens it.
Reset/release first verifies the requester owns the selected channel; a foreign
request returns `PermissionDenied` without touching its window or DMA state.

Reset closes access and awaits DMA's real reset handshake before reopening the
same owner's unconfigured channel. Release closes access and awaits the same
handshake before freeing the allocation. Reuse then programs the new owner and
opens its window. Firmware must retain old memory until reset completes; reset
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

`FunctionControlRegisters(function_count)` repeats complete `ControlRegisters`
blocks. Each block has a 20-byte extent and eight-byte alignment, yielding a
24-byte bank stride. Placement is platform-selected. The firmware configuration
supplies the bank base/count; each Host receives its own block base. Generated
role headers describe the control-block template and `CONTROL_STRIDE`, without
hardcoding a platform Function count or address.

## Scope and evidence

This is management/MMIO/interrupt isolation, **not DMA address-space isolation**.
The memory backend is still shared. Trusted software must keep other Functions'
ring memory intact; DMA and control-memory accesses carry no Function/PASID tags.
Malformed ring or bus failures remain subsystem execution errors, not independently
recoverable Function failures. PCIe integration, PF/VF provisioning, Function reset,
FLR, PASID/context tables, event rings and hostile-memory isolation remain future work.

`firmware_managed/cases/functions.rs` exercises two real IRQ-driven copies and
reset during a partial transfer while peer DMA completes. Runtime boundary tests
exercise cross-Function lifecycle/MMIO rejection, reuse revocation, dormant peers,
and a real DMA notification delivered after reset/regrant. Hardware tests cover
foreign bootstrap/doorbell access, independent pulses and open-binding interlocks.
