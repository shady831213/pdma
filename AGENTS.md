# PROJECT KNOWLEDGE BASE

Generated: 2026-06-29
Commit: a6d47cb
Branch: master
Scope: continue the executable PDMA model work. v0 and v0.5 are complete; current post-v0.5 work is v0.6 sync actor reference and async parity modeling.

## OVERVIEW

This repo is currently being advanced through the Rust PDMA conceptual model. Treat white-board architecture docs as background. `model/docs/v0_model_plan.md` records completed v0 behavior, `model/docs/v0_5_model_plan.md` records the completed workspace boundary refactor, and `model/docs/v0_6_model_plan.md` is the source of truth for the active sync actor reference and async parity work.

## STRUCTURE

```text
pdma/
├── model/docs/v0_model_plan.md    # completed v0 behavior ledger
├── model/docs/v0_5_model_plan.md  # completed v0.5 workspace/boundary plan
├── model/docs/v0_6_model_plan.md  # active v0.6 actor reference and async parity plan
└── model/modeling/                # Rust 2024 model workspace
```

Ignore generated `model/modeling/target/` content.

## WHERE TO LOOK

| Task | Location | Notes |
| --- | --- | --- |
| Determine current slice | `model/docs/v0_6_model_plan.md` | Active actor reference and async parity plan. |
| Shared host/device contract | `model/modeling/crates/contract/` | IDs, DMA object/range vocabulary, descriptors, records, config. |
| Device behavior | `model/modeling/crates/device/` | `PdmaDevice`, scheduler, engines, resource internals, backing store, trace sink. |
| Host software model | `model/modeling/crates/host/` | `Driver`, setup/submit/poll helpers, local slot accounting. |
| System scenarios | `model/modeling/crates/scenarios/` | End-to-end host/device/contract validation stories. |
| Validate workspace | `model/modeling/Cargo.toml` | Workspace lint/profile config. |

## CODE MAP

| Area | Location | Role |
| --- | --- | --- |
| Contract API | `crates/contract/src/` | Shared soft/hard Rust contract and DMA-visible object vocabulary. |
| Device API | `crates/device/src/device.rs` | Device state machine and observable step loop. |
| Device blocks | `crates/device/src/{scheduler.rs,engines.rs,resources.rs,rings.rs}` | Hardware-behavior internals. |
| Host API | `crates/host/src/driver.rs` | SDK/driver-like software model. |
| Scenarios | `crates/scenarios/tests/` | System-level behavioral validation. |

## CURRENT STATE

Completed slices: scaffold, typed IDs, checked memory, generic ring, driver setup, device loop, linear copy, strided copy, event ring, queue config, deterministic arbitration, broadcast copy, vector-add, Resource Credit Model, Backpressure/Stall Annotation Layer, Trace Sink, v0 architecture specification draft.

Active next slice: v0.6 Phase 2 async parity is in progress. The sync path is locked as the reference implementation: host software acts through `DevicePort`/`Driver::*_on`, and primary scenarios use the sync hardware runner rather than explicit device turns. Async crates now mirror that behavior incrementally through async device, host, and scenario crates.

## CONVENTIONS

- Keep v0 deterministic and step-based; do not add cycle timing.
- Add failing scenario tests first, then minimum model behavior.
- Preserve existing behavior for linear, strided, event, queue-config, arbitration, broadcast, vector-add, resource-credit, stall-annotation, and trace-sink slices.
- For v0.6 primary scenarios, use actor-style host calls through `Driver::*_on` and the sync runner's progress/error waits; direct `step()`, `run_until_idle()`, and `run_until_blocked()` calls are only for explicit helper/status/trace/arbitration and step-budget coverage.
- Keep operation descriptors conceptual; exact byte layout, register offsets, C ABI, and generated headers stay deferred.
- Keep Rust files below 250 pure LOC; split by responsibility before growing `desc.rs` or `device.rs`.
- Use English plain short commit messages matching recent history if the user asks to commit.

## ANTI-PATTERNS

- Do not work outside the model workspace unless the user explicitly changes scope.
- Do not stage or document `model/modeling/target/`.
- Treat `docs/white_boards/**` as exploratory background, not active implementation scope.
- Do not add async runtime, unsafe code, descriptor ABI, register maps, or C headers in v0 slices.
- Do not publish success completions after memory faults.
- Do not weaken tests to pass validation.

## COMMANDS

```bash
cd model/modeling
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps --document-private-items
```

Use `GIT_MASTER=1` before every git command.

## NOTES

For v0.6 work, keep crate boundaries aligned with `model/docs/v0_6_model_plan.md`: contract has no backing store, sync and async device crates have no host dependency, scenarios crates have no dependents, and async/runtime dependencies stay out of the sync crates.
