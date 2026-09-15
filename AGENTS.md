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
managed MVP's `hw-components`, `firmware` and `managed-runtime`, system scenarios,
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
and `firmware`, with concrete wiring currently in `managed-runtime`. User review
selected moving that assembly into `scenarios/tests/firmware_managed`; this move
and runtime framework enhancements are deferred to the next session.
The managed MVP has one firmware-opened work window and a separate Host readiness
view; it does not implement Function/PASID mapping or PCIe integration.
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
- Add failing behavioral tests before new model implementation.
- Keep hardware behavior deterministic and step-based, without cycle timing.
- Hardware never stores or invokes software interrupt handlers.
- Keep software CPU-local and interrupt-dispatch safety guarantees intact.
- Keep memory ownership and backend selection outside hardware behavior.
- Keep Rust files below 250 pure LOC and use the workspace lint configuration.
- Do not add unsafe code or weaken tests to pass validation.
- Do not stage build caches or incidental agent state.
- Use `GIT_MASTER=1` before every git command.
- Use short English commit messages matching repository history.
