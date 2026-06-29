# PDMA MODELING CRATE

Scope: Rust v0 conceptual model only.

## OVERVIEW

`pdma-modeling` is a Rust 2024 library crate for deterministic, step-based PDMA behavior slices.

## STRUCTURE

```text
src/
├── lib.rs          # public API and ModelError
├── device.rs       # step loop, runnable queues, completions/events
├── driver.rs       # SDK-like setup/submit/poll surface
├── scheduler.rs    # external command lowering
├── engines.rs      # checked execution engines
├── queue.rs        # queue config/directory metadata
├── rings.rs        # bounded ring primitive
├── desc.rs         # shared external command descriptors/re-exports
├── scheduled.rs    # internal scheduled work descriptors
├── broadcast.rs    # broadcast-specific descriptors
└── vector_add.rs   # vector-add-specific descriptors
```

`Cargo.toml`, `clippy.toml`, and `rustfmt.toml` define the local lint and format contract.

## WHERE TO LOOK

| Task | Location | Notes |
| --- | --- | --- |
| New operation API | `desc.rs` plus an operation module | Re-export from `desc.rs`; split bulky types into their own file. |
| Queue/resource behavior | `device.rs`, `queue.rs`, `rings.rs` | Backpressure/stall and trace behavior should start here. |
| Lowering changes | `scheduler.rs`, `scheduled.rs` | Use exhaustive `match` on owned enums. |
| Engine behavior | `engines.rs`, `memory.rs` | Checked reads/writes only; map memory faults to events. |
| Driver scenarios | `tests/*.rs` | Drive through `Driver` and `PdmaDevice`, not private internals. |

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
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo doc --no-deps --document-private-items
```

## CURRENT NEXT SLICE

Trace Sink:

- Emit Chrome/Perfetto-compatible JSON trace events for command lifecycles, completions, events, and stall annotations.
- Keep a structured in-memory event log that scenario tests can inspect without timing assumptions.
- Preserve existing linear, strided, event, queue-config, arbitration, broadcast, vector-add, resource-credit, and stall-annotation behavior when tracing is enabled or disabled.
- Add `tests/trace_sink.rs` first.

## DO NOT

- Do not add exact descriptor byte layout, register offsets, C ABI, generated headers, or cycle timing.
- Do not add `unsafe`, `unwrap`, `expect`, `panic`, `todo`, or indexing/slicing in production code.
- Do not grow any Rust source beyond 250 pure LOC; split before adding behavior.
