# PROJECT KNOWLEDGE BASE

Generated: 2026-06-29
Commit: a6d47cb
Branch: master
Scope: only continue the executable PDMA v0 model work.

## OVERVIEW

This repo is currently being advanced through the Rust PDMA v0 conceptual model. Treat white-board architecture docs as background; `model/docs/v0_model_plan.md` is the source of truth for active work.

## STRUCTURE

```text
pdma/
├── model/docs/v0_model_plan.md  # progress ledger and active next slice
└── model/modeling/              # Rust 2024 step-based executable model
```

Ignore generated `model/modeling/target/` content.

## WHERE TO LOOK

| Task | Location | Notes |
| --- | --- | --- |
| Determine current slice | `model/docs/v0_model_plan.md` | Read before coding; update when a slice lands. |
| Implement model behavior | `model/modeling/src/` | Keep deterministic `step()` and `run_until_idle()` semantics. |
| Add scenario coverage | `model/modeling/tests/` | One narrative test per observable behavior or fault path. |
| Validate Rust crate | `model/modeling/Cargo.toml` | Strict lint profile; no dependencies currently. |

## CODE MAP

| Area | Location | Role |
| --- | --- | --- |
| Public surface and errors | `model/modeling/src/lib.rs` | Re-exports conceptual API; owns `ModelError`. |
| Driver path | `model/modeling/src/driver.rs` | Queue setup, submission, completion/event polling. |
| Device loop | `model/modeling/src/device.rs` | Queue arbitration, one-command step, completion/event publication. |
| Lowering | `model/modeling/src/scheduler.rs` | External commands to internal scheduled work. |
| Execution | `model/modeling/src/engines.rs` | Checked memory operations and compute behavior. |
| Resource metadata | `model/modeling/src/queue.rs`, `rings.rs` | Queue config, ring capacity, completion/event pressure basis. |
| Descriptors | `desc.rs`, `scheduled.rs`, `broadcast.rs`, `vector_add.rs` | Keep bulky operation-specific descriptors split out of `desc.rs`. |

## CURRENT STATE

Completed slices: scaffold, typed IDs, checked memory, generic ring, driver setup, device loop, linear copy, strided copy, event ring, queue config, deterministic arbitration, broadcast copy, vector-add, Resource Credit Model, Backpressure/Stall Annotation Layer.

Active next slice: Trace Sink with Chrome/Perfetto-compatible JSON events plus a structured event log.

## CONVENTIONS

- Keep v0 deterministic and step-based; do not add cycle timing.
- Add failing scenario tests first, then minimum model behavior.
- Preserve existing behavior for linear, strided, event, queue-config, arbitration, broadcast, vector-add, resource-credit, and stall-annotation slices.
- Keep operation descriptors conceptual; exact byte layout, register offsets, C ABI, and generated headers stay deferred.
- Keep Rust files below 250 pure LOC; split by responsibility before growing `desc.rs` or `device.rs`.
- Use English plain short commit messages matching recent history if the user asks to commit.

## ANTI-PATTERNS

- Do not work on non-v0-model areas unless the user explicitly changes scope.
- Do not stage or document `model/modeling/target/`.
- Treat `docs/white_boards/**` as exploratory background, not active implementation scope.
- Do not add async runtime, unsafe code, descriptor ABI, register maps, or C headers in v0 slices.
- Do not publish success completions after memory faults.
- Do not weaken tests to pass validation.

## COMMANDS

```bash
cd model/modeling
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
cargo doc --no-deps --document-private-items
```

Use `GIT_MASTER=1` before every git command.

## NOTES

After each slice, update `model/docs/v0_model_plan.md`: move the slice to the progress ledger, promote exactly one active next slice, and keep deferred decisions in the deferred section.
