# PDMA REBUILD WORKSPACE

## Current state

The user authorized a clean PDMA restart on 2026-09-09.
The previous implementation, including the last uncommitted memory refactor and
the retired sync model, is preserved on:

- Branch: `backup/pdma-before-rebuild-20260909`
- Commit: `51707fe1d38e261a188e1d0ac14243c0caf58127`

The replacement `contract`, `device`, and `host` implement a minimal single channel.
The old implementations and `model/retired` remain only on the backup branch.
Do not restore their compatibility interfaces without a user request.

## Scope and retained material

Active work is under `model/modeling`; read its `AGENTS.md`.
Its Cargo workspace contains the three minimal PDMA crates and four reusable building blocks:

- `hardware-interface-pipeline`
- `hardware-interface-runtime`
- `hardware-interface-schema`
- `hardware-interface-schema-macros`

The original `crates/scenarios` sources are preserved on the backup branch only.
Current acceptance uses the active crate tests and the `minimal_channel` host example.
The version plans, `model/docs/pdma_arch_spec_v0.md`, and generated PDMA header describe
the old implementation, not the replacement's required architecture or ABI.
`docs/white_boards/**` remains exploratory background.

The current implementation has one channel with command/response rings, bounded DMA
copy, and interrupt delivery using existing building blocks. Do not expand that scope or
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
