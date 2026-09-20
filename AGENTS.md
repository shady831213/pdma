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
DMA tags accesses with channel metadata; runtime resolves peripheral owner registers
and selects Function backing. Backing and software allocators remain platform-owned. No PASID/IOMMU translation,
Function reset/FLR, event ring or PCIe integration is implemented.
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
building blocks. Do not expand that scope or
recreate the old API surface to satisfy historical tests.

## Working rules

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
