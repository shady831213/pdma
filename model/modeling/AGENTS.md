# PDMA MODELING WORKSPACE

Scope: Rust v0.5 conceptual model workspace only.

## OVERVIEW

`model/modeling` is a Rust 2024 workspace for deterministic, step-based PDMA behavior slices. The former flat `pdma-modeling` crate has been split into contract, device, host, and scenarios crates.

## STRUCTURE

```text
crates/
├── contract/   # shared host/device contract and DMA-visible object vocabulary
├── device/     # hardware-behavior model, backing store, scheduler, engines, trace sink
├── host/       # SDK/driver-like software model
└── scenarios/  # system-level host/device/contract validation stories
```

Workspace `Cargo.toml`, `clippy.toml`, and `rustfmt.toml` define the lint and format contract.

## WHERE TO LOOK

| Task | Location | Notes |
| --- | --- | --- |
| Shared contract | `crates/contract/src/` | No backing memory or device execution behavior. |
| Device behavior | `crates/device/src/` | Device owns backing store, scheduler, engines, resources, and trace sink. |
| Host software | `crates/host/src/` | Host owns `Driver` and local command-slot accounting. |
| System scenarios | `crates/scenarios/tests/` | Drive through `Driver` and `PdmaDevice`, not private internals. |

## CONVENTIONS

- Tests use Given/When/Then comments and assert observable completions, events, and memory contents.
- `PdmaDevice::step()` executes at most one command; `run_until_idle(max_steps)` is the scenario surface.
- `Driver` owns local command-slot accounting; completions/events reclaim slots.
- Memory faults publish events and prevent success completions; later queued commands remain runnable where existing tests require it.
- New public structs need terse docs because `missing_docs` is enabled.
- Prefer typed request structs/newtypes over long argument lists or clippy suppressions.
- `clippy.toml` caps constructors at 6 arguments and functions at 100 lines; tests may use `expect`/`panic`, production may not.
- `rustfmt.toml` uses Rust 2024, 100-column width, Unix newlines, and reordered imports/modules.

## VALIDATION

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps --document-private-items
```

## CURRENT NEXT SLICE

v0.5 boundary refactor: keep exactly four crates (`contract`, `device`, `host`, `scenarios`) and no transitional facade crate.

## DO NOT

- Do not add exact descriptor byte layout, register offsets, C ABI, generated headers, or cycle timing.
- Do not add `unsafe`, `unwrap`, `expect`, `panic`, `todo`, or indexing/slicing in production code.
- Do not grow any Rust source beyond 250 pure LOC; split before adding behavior.
- Do not put backing memory in contract; keep DMA-visible data structures separate from storage media.
- Do not make device depend on host or scenarios.
