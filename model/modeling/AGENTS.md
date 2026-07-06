# PDMA MODELING WORKSPACE

Scope: Rust v0.6.5 active async queue-port model workspace.

## OVERVIEW

`model/modeling` is a Rust 2024 workspace for deterministic, step-based PDMA behavior slices. The old sync reference stack has been retired to `model/retired/v0_6_sync_reference/`; this workspace now contains the active contract, device, host, and scenarios crates.

## STRUCTURE

```text
crates/
├── contract/         # shared host/device contract and DMA-visible object vocabulary
├── device/           # Tokio-backed active hardware runner, worker, and handle
├── host/             # active SDK/driver-like software model
└── scenarios/        # active behavior-match validation stories
```

Workspace `Cargo.toml`, `clippy.toml`, and `rustfmt.toml` define the lint and format contract.

## WHERE TO LOOK

| Task | Location | Notes |
| --- | --- | --- |
| Shared contract | `crates/contract/src/` | No backing memory or device execution behavior. |
| Device behavior | `crates/device/src/` | Active queue-port device model, async actor handle/worker, sync/manual drive facade, resources, and trace sink. |
| Host software | `crates/host/src/` | Host owns `AsyncDriver` and local command-slot accounting. |
| System scenarios | `crates/scenarios/tests/` | Drive behavior through `AsyncDriver::*_on`, `AsyncDeviceHandle`, and caller-owned worker execution. |

## CONVENTIONS

- Tests use Given/When/Then comments and assert observable completions, events, and memory contents.
- `AsyncDevice::queue_port*()` returns `(AsyncDevice, AsyncDeviceWorker)`; callers choose whether to `tokio::spawn(worker.run())`, directly await it, or use another executor strategy.
- `SyncDevice::run_until_idle(max_steps)` and `SyncDevice::run_until_blocked(max_steps)` calls belong only in explicit manual-drive helper/status/trace and step-budget tests.
- `AsyncDriver` owns local command-slot accounting; completions/events reclaim slots.
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

v0.6.5 active-model cleanup is the current state: old sync crates are retired, active crates use canonical names, and spawn policy lives in scenarios or higher-level callers.

## DO NOT

- Do not add exact descriptor byte layout, register offsets, C ABI, generated headers, or cycle timing.
- Do not modify retired sync crates except for deliberate rollback-snapshot maintenance requested by the user.
- Do not add `unsafe`, `unwrap`, `expect`, `panic`, `todo`, or indexing/slicing in production code.
- Do not grow any Rust source beyond 250 pure LOC; split before adding behavior.
- Do not put backing memory in contract; keep DMA-visible data structures separate from storage media.
- Do not make device depend on host or scenarios.
