# PDMA REBUILD WORKSPACE

## Current state

The user authorized a clean PDMA restart on 2026-09-09.
The previous implementation, including the last uncommitted memory refactor and
the retired sync model, is preserved on:

- Branch: `backup/pdma-before-rebuild-20260909`
- Commit: `51707fe1d38e261a188e1d0ac14243c0caf58127`

The replacement `contract`, `dma`, and `host` implement minimal statically configured channels.
The old implementations and `model/retired` remain only on the backup branch.
Do not restore their compatibility interfaces without a user request.

## Scope and retained material

Active work is under `model/modeling`; read its `AGENTS.md`.
Its Cargo workspace contains the core `contract`, `dma` and `host` crates, the
managed MVP's `hw-components` and `firmware`, shared PDMA `software`, system scenarios,
and four reusable building blocks:

- `hardware-interface-pipeline`
- `hardware-interface-runtime`
- `hardware-interface-schema`
- `hardware-interface-schema-macros`

The original scenarios sources are preserved on the backup branch only.
The new `crates/scenarios` contains current-model integration tests. Current acceptance
uses these system scenarios and the active crates' boundary tests.
The obsolete architecture specification and v0.x plans were removed from
`model/docs`; use Git history for those documents, not as current requirements.
Current architecture and reset documentation live under `model/modeling`.
`model/modeling/docs/firmware-dma-goal.md` records the next major direction,
not implemented behavior or an automatic implementation order.
`model/modeling/docs/firmware-dma-mvp.md` defines the approved architecture-validation
milestone before broader feature work. The implemented split is `dma`, `hw-components`
and `firmware`, with concrete wiring in `scenarios/tests/firmware_managed/harness`.
The standalone managed-runtime crate was removed after user review. Ordinary and
managed transports share the framework's split pending-interrupt capabilities.
DMA disables the framework's default async-runtime feature; its standalone normal
dependency tree has no Tokio. This is not a separate memory-only interface crate.
The managed assembly has independently firmware-managed channel windows, independent boot status,
and separate in-memory control command/response rings with bootstrap and bidirectional
doorbell registers. Host and firmware share `software::ring::SoftRing`; doorbell waits
are independent of DMA interrupt dispatch. Ready leaves DMA closed; Host requests
a channel and releases it through firmware, which serially awaits real DMA reset before
acknowledging reset or release. One PF management client pipelines bounded asynchronous
requests naming target Functions over one control ring pair. VF ports only operate granted
DMA data channels. Firmware owns Function-to-channel
allocation; peripheral registers enforce MMIO ownership and route channel notifications.
See `model/modeling/docs/{control-channel,function-isolation}.md`. Managed DMA can
use independent Function memory capabilities or an explicitly shared backend.
DMA tags accesses with channel/context selectors; hw-components resolves owner
registers and optionally an instance-sized context table. Context attributes are
an extensible schema record, initially Default/PASID (Default differs from PASID zero).
Runtime selects injected Function/address-space backing; allocation stays platform-owned.
PF control memory has its own enable-frozen configuration, not a reserved table slot.
Firmware supports Create/Drain/Abort/DestroyContext through its existing control queues.
Hardware accepts an atomic context-ID plus Drain/Abort request; firmware implements
Destroy by awaiting Drain success and explicitly clearing valid. A retained failed-channel
bitmap is the final context-map bank and covers all detected failures in the operation scope.
DMA captures producer frontiers, discards unstarted or parked Abort targets and retains
accepted task references through suspension until terminal retirement. Running Abort
targets remain command-atomic unless they park. Drain encountering parked work returns
ChannelStopped with the retained failure bitmap. Failure retains the binding
and permits channel-reset recovery. See model/modeling/docs/context-contract.md for
mandatory software lifetime/submission obligations. No PASID/IOMMU translation,
Function reset/FLR or PCIe integration is implemented.
Managed firmware handles DMA fatal notifications, closes work windows and publishes
PF events over an independent in-memory event ring and event doorbell. Event failure
is retained in PF-readable firmware-owned status without retries or backpressure.
ResetDma preserves grants/bindings and management state but leaves work windows closed;
ResetEvents rearms only a failed event producer. See docs/spec.md and docs/fatal-events.md
under model/modeling for system requirements, including firmware and RTL obligations.
PF-only firmware management retains per-Function work MMIO and IRQ isolation.
Hardware checks the platform-selected PF identity on every control-register access;
firmware validates request targets and channel ownership. The agreed future VF FLR
contract requires PF to release all target channels successfully before FLR and
prevent reacquisition until FLR completes. No firmware FLR ingress or repeated
cleanup is needed: application ACK may be immediate under that precondition, while
PCIe drain/controller handshake stays at integration. Direct FLR without preparation
is unsupported, with no automatic check or recovery. PF FLR is a separate scope;
FLR remains unimplemented. See `model/modeling/docs/function-isolation.md`.
The generated header under `model/modeling/include` describes the current contract.
`docs/white_boards/**` remains exploratory background.

The current implementation has an instance-configured channel count with independent reset,
command/response rings and one shared DMA execution pipeline. It supports independently
selected linear/scatter-gather DMA endpoints, and interrupt delivery using existing
building blocks. Recoverable payload PageFault parks the original command and both
cursors, including successful read scratch for a failed write, while peers can run.
Owner software repairs through its platform, observes FAULT_STATUS.faulted and
writes RESUME.request=1 in channel work MMIO; both registers are 32 bits. No fault
response or DMA IRQ/event is produced. Software must serialize repair/resume with
resource lifecycle operations. Platform-defined opaque tags distinguish fault
sources even when channels share a Function/PASID; the MMU echoes tags rather than
interpreting PDMA channel IDs. hw-components exposes authorized source/identity
sideband mapping; MemoryIdentity stays address-space-only. Tags must retain their
association through outstanding reports/handlers. Software must coordinate
reset, release and reallocation and retire old fault handling before reuse; hardware
does not correlate late or duplicate requests with fault episodes. Normal responses are 24 bytes with
statuses 0..5; retired PageFault status 6 is invalid. See
`model/modeling/docs/page-fault-recovery.md`. No true MMU, PCIe integration,
oversubscription or barrier semantics are added. Do not expand that scope or
recreate the old API surface to satisfy historical tests.

## Working rules

The 2026-10-21 decision withdraws the first Signal/Wait feature while retaining
generic memory atomics and reset during pending context maintenance. Use the
copy-engine reassessment in `model/modeling/docs/firmware-dma-goal.md` for next-step
decisions. HSA is a semantic reference; AQL compatibility is not a current goal.

- Use native Rust module paths. `#[path]` and conditional path redirection are forbidden;
  do not bypass this with `include!`, symlinks, or duplicate source trees.
- Preserve retained building-block behavior and tests.
- Keep PDMA wire/protocol rules in `contract` and shared software algorithms in
  `software`. An `Entry` type parameter alone does not justify framework placement.
  Generic schema/runtime crates expose no PDMA ring protocol or ring types and have
  no normal dependency on PDMA crates; transport, CPU and notification mechanics
  remain in the generic runtime.
- Add failing behavioral tests before new model implementation.
- Keep hardware behavior deterministic and step-based, without cycle timing.
- Hardware never stores or invokes software interrupt handlers.
- Keep explicit CPU-local resource and interrupt-dispatch safety guarantees intact.
- Do not impose CPU affinity on ordinary drivers or rings. CPU-specific resource
  checks and main/ISR scheduling belong to runtime; queue ownership is not CPU identity.
- Keep memory ownership and backend selection outside hardware behavior.
- Keep Rust files below 250 pure LOC and use the workspace lint configuration.
- Do not add unsafe code or weaken tests to pass validation.
- Do not stage build caches or incidental agent state.
- Use `GIT_MASTER=1` before every git command.
- Use short English commit messages matching repository history.
