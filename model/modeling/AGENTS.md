# PDMA MODELING WORKSPACE

Scope: Rust workspace with the completed v0.9 ABI register-contract model.

## OVERVIEW

`model/modeling` is a Rust 2024 workspace for deterministic, step-based PDMA behavior
slices. The old sync reference stack has been retired to `model/retired/v0_6_sync_reference/`;
this workspace now contains the active contract, device, host, and scenarios crates.

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
| Shared contract | `crates/contract/src/` | ABI-v1 metadata/codecs and vocabulary; no backing memory, runtime, handler traits, or device execution. |
| Device behavior | `crates/device/src/` | Queue-port model, checked `DmaSpace`, typed bus controls/reset, shared execution/retirement, async worker, trace, and PDMA-local IRQ. |
| Host-side model | `crates/host/src/` | CPU-bound `AsyncDriver`, manual facade, `AsyncPdmaBus`, `BusDevicePort`, IRQ delivery, and SW context helpers. |
| System scenarios | `crates/scenarios/tests/` | Concurrent actors; `message_irq_linear_copy` is the canonical ABI Message-IRQ E2E. |

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
- Await-spanning state private to one vector handler belongs in its persistent
  `StatefulIrqHandler`; do not shuttle handler-owned resources through
  `CpuLocal<Option<T>>` on every IRQ turn.
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
- The scenario invariant is 56 tests: 51 software-bearing tests with one `run_main` and
  five pure device/manual observability exceptions with none. Do not add artificial
  software actors to those exceptions.
- ABI-v1 is the canonical frontend for new E2E scenarios. `AsyncPdmaBus` and
  `BusDevicePort` add checked DMA/control transactions without a second executor; the
  semantic and ABI frontends share execution and retirement behavior.
- A successful whole-device reset checked-advances `DeviceEpoch`, preserves committed
  `DmaSpace` mappings/bytes/generation, trace history, resource configuration, and
  lifecycle accounting, and clears queue/control/runnable/result/pending-retirement/
  PDMA-IRQ state. Prior-epoch queue, IRQ, and notification capabilities reject before
  mutation and require explicit new-epoch bootstrap; epoch overflow does not mutate.
- `include/pdma_model_abi.h` is a generated C11 data header, not a callable C API. Change
  it only with `cargo run -p pdma-model-contract --bin pdma-abi-header -- --write
  include/pdma_model_abi.h`, then require the matching `--check` command in validation.
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

v0.9 ABI Register-Contract Model is complete. `model/docs/v0_9_model_plan.md` records
the ABI-v1 canonical E2E path, shared execution/retirement core, checked DMA/control
transactions, whole-device reset epochs, and generated-header policy. v0.8 records the
still-binding vector-to-handler, CPU-local, cancel-safe accounting, bootstrap, and
main/handler alternation boundaries; v0.7 is historical. No next implementation slice is
selected in this workspace knowledge base.

## DO NOT

- Do not add numeric register/MMIO maps, callable C APIs, Terminus, PCIe/MSI-X numeric
  maps or table/PBA behavior, SR-IOV, FLR, per-queue reset, a second executor/driver/E2E,
  or cycle timing without an explicitly selected new slice. Do not hand-edit the generated
  ABI data header.
- Do not modify retired sync crates except for deliberate rollback-snapshot maintenance requested by the user.
- Do not add `unsafe`, `unwrap`, `expect`, `panic`, `todo`, or indexing/slicing in production code.
- Do not grow any Rust source beyond 250 pure LOC; split before adding behavior.
- Do not put backing memory in contract; keep DMA-visible data structures separate from storage media.
- Do not make device depend on host or scenarios.
- Do not make the device store handler tables or invoke software callbacks.
- Do not add IRQ service to `ManualDriver` or bypass `AsyncDriver` CPU validation.
