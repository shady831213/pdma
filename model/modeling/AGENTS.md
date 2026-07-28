# PDMA MODELING WORKSPACE

Scope: Rust workspace with the completed v0.8 vector-routed IRQ context model.

## OVERVIEW

`model/modeling` is a Rust 2024 workspace for deterministic, step-based PDMA behavior slices. The old sync reference stack has been retired to `model/retired/v0_6_sync_reference/`; this workspace now contains the active contract, device, host, and scenarios crates.

## STRUCTURE

```text
crates/
├── contract/         # shared host/device contract, DMA-visible vocabulary, and IRQ types
├── device/           # active hardware runner plus PDMA-local IRQ controller
├── host/             # driver, device port, and host-owned IRQ delivery actor
└── scenarios/        # active host/device/IRQ-delivery validation stories
```

Workspace `Cargo.toml`, `clippy.toml`, and `rustfmt.toml` define the lint and format contract.

## WHERE TO LOOK

| Task | Location | Notes |
| --- | --- | --- |
| Shared contract | `crates/contract/src/` | No backing memory, runtime, handler traits, or device execution behavior. |
| Device behavior | `crates/device/src/` | Active queue-port device model, async actor handle/worker, sync/manual drive facade, resources, trace sink, and PDMA-local IRQ controller. |
| Host-side model | `crates/host/src/` | Host owns CPU-bound `AsyncDriver`, CPU-unbound `ManualDriver`, device port traits, IRQ delivery actor, and SW execution context helpers. |
| System scenarios | `crates/scenarios/tests/` | Drive behavior through concurrent device worker, SW main future, and IRQ delivery future. |

## CONVENTIONS

- Tests use Given/When/Then comments and assert observable completions, events, and memory contents.
- `AsyncDevice::queue_port*()` returns `(AsyncDevice, AsyncDeviceWorker)`; callers choose whether to `tokio::spawn(worker.run())`, directly await it, or use another executor strategy.
- `SyncDevice::run_until_idle(max_steps)` and `SyncDevice::run_until_blocked(max_steps)` calls belong only in explicit manual-drive helper/status/trace and step-budget tests.
- `AsyncDriver` is CPU-bound: setup requires an active matching `SoftwareCpu`, clones share
  one command-ID/slot accounting state, and it alone services IRQs. `ManualDriver` is the
  explicit CPU-unbound observability facade, constructed with `ManualDriver::setup_on` or
  `ManualDriver::setup_with_config_on`; it has no IRQ service. Both public facades forward
  to one private core with clone-shared accounting. Among system scenarios, only the five
  named zero-entry observability tests use manual setup; focused host tests may exercise it.
- Device-local IRQ enable/mask/ack belongs to `pdma-model-device`; handler registration and delivery adaptation belongs to `pdma-model-host`.
- Software handlers register by `IrqVector` with host IRQ delivery before `run`, never
  with the device. Multiple sources may share one vector; each vector has one persistent
  handler entry, and delivery remains serial and non-reentrant.
- `CpuLocal<T>` is the modeled single-copy mutable software-state capability. Access is
  synchronous and valid only during a poll of the owning CPU's main or handler future;
  do not expose a lock, guard, raw CPU identity, or await-spanning borrow.
- The private `SoftwareCpuToken` is installed in the single `CURRENT_SOFTWARE_CPU` option
  only around an actual inner main or handler poll. RAII restores any prior token after the
  poll, so a nested CPU context does not erase its enclosing context.
- Before every nested poll of a CPU-bound create, submit, doorbell, completion, or event
  port future, validate the owning active CPU. A pending future resumed inactive or on
  another CPU returns the typed error before its commit point. `AsyncDevicePort` commits
  create only on `Ok(_)`, submit/doorbell only on `Ok(())`, record removal only on
  `Ok(Some(_))`, and preserves the corresponding device state when dropped pending.
- Each software-bearing scenario calls `SoftwareCpu::run_main` once with its complete
  program. One poll is one atomic main segment; only `Poll::Pending` releases the CPU
  before the next poll, while the complete IRQ handler future remains atomic.
- CPU gate ordering is FIFO only after a contender requests it. Do not claim priority
  for an unobserved IRQ notification or allow a later IRQ request to overtake queued main.
- Actor construction/spawn/shutdown/join, trace instrumentation/collection, and final
  assertions belong to the harness. The disabled-submission trace edge test alone keeps
  trace enable/clear inside main to preserve its deliberate interleaving.
- The scenario invariant is 55 tests: 50 software-bearing tests with one `run_main` and
  five pure device/manual observability exceptions with none. Do not add artificial
  software actors to those exceptions.
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

## CURRENT STATE

v0.8 Vector-Routed IRQ Context Model is complete. `model/docs/v0_8_model_plan.md`
records vector-to-handler dispatch, CPU-local single-copy state, cancel-safe shared driver
accounting, bootstrap ordering, and repeated main/handler alternation. The v0.7 plan is a
historical single-handler snapshot. No next implementation slice is selected in this
workspace knowledge base.

## DO NOT

- Do not add exact descriptor byte layout, register offsets, C ABI, generated headers, or cycle timing.
- Do not modify retired sync crates except for deliberate rollback-snapshot maintenance requested by the user.
- Do not add `unsafe`, `unwrap`, `expect`, `panic`, `todo`, or indexing/slicing in production code.
- Do not grow any Rust source beyond 250 pure LOC; split before adding behavior.
- Do not put backing memory in contract; keep DMA-visible data structures separate from storage media.
- Do not make device depend on host or scenarios.
- Do not make the device store handler tables or invoke software callbacks.
- Do not add IRQ service to `ManualDriver` or bypass `AsyncDriver` CPU validation.
