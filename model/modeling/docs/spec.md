# PDMA system specification

This is the entry point for the active system specification. The specified system
includes DMA hardware, peripheral hardware and firmware. Host drivers and the
integration platform have explicit obligations; firmware is not an unspecified
external driver. The executable model and its scenarios validate observable
behavior, not a particular RTL microarchitecture.

## Scope and authority

The direct configuration contains DMA alone. The managed configuration adds
Function access windows, context resolution, management registers and firmware.
Managed requirements must not change the direct configuration's interfaces.
PCIe, IOMMU translation, caches and an actual interconnect are integration scope.

| Chapter | Normative subject |
| --- | --- |
| [Reset](reset.md) | Stop, retire, drain and reset completion; memory lifetime |
| [Control channel](control-channel.md) | PF/firmware requests, responses, ring ownership and cancellation |
| [Function isolation](function-isolation.md) | Grants, routing, permissions and pre-FLR obligations |
| [Contexts](context-contract.md) | Context bindings, maintenance frontiers, lifetime and non-faulting migration-copy obligations |
| [Fatal and events](fatal-events.md) | System fatal handling, PF events and managed recovery |

The fatal/event chapter describes the managed recovery extension. Its verification
section records implementation coverage. Other
chapters describe the existing model except sections explicitly marked future.
The architecture goal and MVP documents are background, not additional acceptance
requirements. Removed specifications on the backup branch are not authoritative.

The agreed [non-faulting migration-copy usage](context-contract.md#non-faulting-page-migration-copy)
uses existing copy behavior with explicit software/platform preconditions. It is
distinct from copying pageable virtual addresses with recoverable faults. The
next design focus is [DSA-like partial completion and Host continuation](page-fault-recovery.md).
That note is non-normative and unimplemented: completion encoding, SG recovery,
ordering and lifecycle interactions must be agreed before adding requirements or
changing the existing error/halt behavior.

## Responsibility notation

- `HW-*`: DMA or peripheral hardware behavior, including visible register semantics.
- `FW-*`: required firmware behavior, part of this system specification.
- `HOST-*`: PF/VF software obligations and permitted interaction sequences.
- `PLAT-*`: integration guarantees supplied outside this system.
- `MODEL-*`: executable-model mechanics that are not RTL requirements.

Each new behavior must name its owner, trigger, observable result, failure result,
ordering and recovery boundary. Existing numbered obligations in the linked
chapters retain their identifiers. A successful test cannot waive a software or
platform precondition.

## RTL handoff

Contract schemas define relative register placement, field access, wire encodings
and instance-sized banks. Generated Host and firmware headers are projections of
the same contract, not independent ABI definitions. Platform bases remain inputs.

RTL must preserve the specified ordering of descriptor acceptance, memory side
effects, response publication, notifications and reset completion. A Rust memory
call completes within one model step; an RTL port may have multiple outstanding
transactions. Before reporting reset/drain complete it must retire **all** affected
transactions and retained work, not only the currently visible request. Neither
reset nor fatal implies rollback of externally completed writes.

RTL design must additionally fix bus protocols, CDC, outstanding transaction
accounting, reset domains, physical interrupt transport and cache-ordering
mechanisms. Those decisions must implement the platform obligations here; the
absence of cycles, caches or transactions in this model is not evidence that they
are unnecessary. Verification links in each chapter connect requirements to model
tests and identify integration properties that still require RTL verification.
