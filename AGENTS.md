# PROJECT KNOWLEDGE BASE

Generated: 2026-07-07
Commit: v0.6.5 pushed to origin/master
Branch: master
Scope: continue the executable PDMA model work. v0.6.5 is complete; current work is v0.7 host-owned IRQ delivery modeling.

## OVERVIEW

This repo is currently being advanced through the Rust PDMA conceptual model. Treat white-board architecture docs as background. `model/docs/v0_model_plan.md` records completed v0 behavior, `model/docs/v0_5_model_plan.md` records the completed workspace boundary refactor, `model/docs/v0_6_model_plan.md` records the completed actor/async transition and v0.6.5 active-model cleanup, and `model/docs/v0_7_model_plan.md` is the source of truth for active IRQ delivery work.

## STRUCTURE

```text
pdma/
├── model/docs/v0_model_plan.md    # completed v0 behavior ledger
├── model/docs/v0_5_model_plan.md  # completed v0.5 workspace/boundary plan
├── model/docs/v0_6_model_plan.md  # completed v0.6 actor/async transition and v0.6.5 cleanup
├── model/docs/v0_7_model_plan.md  # active v0.7 host-owned IRQ delivery plan
└── model/modeling/                # Rust 2024 model workspace
```

Ignore generated `model/modeling/target/` content.

## WHERE TO LOOK

| Task | Location | Notes |
| --- | --- | --- |
| Determine current slice | `model/docs/v0_7_model_plan.md` | Active IRQ delivery plan and boundaries. |
| Shared host/device contract | `model/modeling/crates/contract/` | IDs, DMA object/range vocabulary, descriptors, records, config, and IRQ vocabulary. |
| Device behavior | `model/modeling/crates/device/` | Active queue-port device actor, worker, handle, manual facade, resources, trace sink, and PDMA-local IRQ controller. |
| Host-side model | `model/modeling/crates/host/` | Driver, device port, host IRQ delivery actor, and SW execution contexts. |
| System scenarios | `model/modeling/crates/scenarios/` | End-to-end host/device/IRQ-delivery validation stories. |
| Validate workspace | `model/modeling/Cargo.toml` | Workspace lint/profile config. |

## CODE MAP

| Area | Location | Role |
| --- | --- | --- |
| Contract API | `crates/contract/src/` | Shared soft/hard Rust contract and DMA-visible object vocabulary. |
| Device API | `crates/device/src/runner.rs` | `AsyncDevice`, `AsyncDeviceWorker`, `AsyncDeviceHandle`, and `SyncDevice` public surfaces. |
| Device blocks | `crates/device/src/runner/native*.rs` | Queue-port hardware-behavior internals. |
| Host API | `crates/host/src/` | Driver, port, and IRQ delivery modules. |
| Scenarios | `crates/scenarios/tests/` | System-level behavioral validation. |

## CURRENT STATE

Completed slices: scaffold, typed IDs, checked memory, generic ring, driver setup, device loop, linear copy, strided copy, event ring, queue config, deterministic arbitration, broadcast copy, vector-add, Resource Credit Model, Backpressure/Stall Annotation Layer, Trace Sink, v0 architecture specification draft, v0.6 actor/async transition, and v0.6.5 active-model cleanup.

Active next slice: v0.7 Host-Owned IRQ Delivery Model. The active workspace now has canonical `pdma-model-contract`, `pdma-model-device`, `pdma-model-host`, and `pdma-model-scenarios` crates. The old sync stack is retired under `model/retired/v0_6_sync_reference/`. v0.7 adds a PDMA-local IRQ controller in the device crate and a host-owned IRQ delivery actor in the host crate.

## CONVENTIONS

- Keep the model deterministic and step-based; do not add cycle timing.
- Add failing scenario tests first, then minimum model behavior.
- Preserve existing behavior for linear, strided, event, queue-config, arbitration, broadcast, vector-add, resource-credit, stall-annotation, and trace-sink slices.
- For v0.7 IRQ scenarios, run the device worker, SW main future, and host IRQ delivery future as explicit concurrent actors.
- Do not register or invoke software handlers from the device loop; handlers belong to host IRQ delivery.
- Keep operation descriptors conceptual; exact byte layout, register offsets, C ABI, and generated headers stay deferred.
- Keep Rust files below 250 pure LOC; split by responsibility before growing `desc.rs` or `device.rs`.
- Use English plain short commit messages matching recent history if the user asks to commit.

## ANTI-PATTERNS

- Do not work outside the model workspace unless the user explicitly changes scope.
- Do not stage or document `model/modeling/target/`.
- Treat `docs/white_boards/**` as exploratory background, not active implementation scope.
- Do not add unsafe code, descriptor ABI, register maps, or C headers in v0.7 slices.
- Do not collapse device-local IRQ masking/ack into system delivery masking/completion; keep the two layers distinct.
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

For v0.7 work, keep crate boundaries aligned with `model/docs/v0_7_model_plan.md`: contract has vocabulary only, device has no host dependency and never invokes handlers, host owns IRQ delivery, and scenarios wire the actors together.
