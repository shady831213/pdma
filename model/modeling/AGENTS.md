# PDMA MODELING WORKSPACE

Scope: Rust workspace with the COMPLETE v0.9 historical data ABI and exact 4 KiB numeric
MMIO memory-and-bus contract.

## OVERVIEW

`model/modeling` is a Rust 2024 workspace for deterministic, step-based PDMA behavior
slices. The old sync reference stack has been retired to `model/retired/v0_6_sync_reference/`;
this workspace now contains the active contract, device, host, scenarios, and reusable
hardware-interface-schema crates.

## STRUCTURE

```text
crates/
├── contract/         # shared host/device contract, DMA-visible vocabulary, and IRQ types
├── device/           # active hardware runner plus PDMA-local IRQ controller
├── hardware-interface-schema/ # reusable runtime-neutral record/register schema facade
├── hardware-interface-schema-macros/ # proc-macro implementation for the schema facade
├── hardware-interface-runtime/ # shared memory/register/CPU/interrupt/trace runtime
├── hardware-interface-pipeline/ # reusable typed stage composition and driving policies
├── host/             # driver, canonical register port, PDMA IRQ adapter, and compat ports
└── scenarios/        # active host/device/IRQ-delivery validation stories
```

Workspace `Cargo.toml`, `clippy.toml`, and `rustfmt.toml` define the lint and format contract.

## WHERE TO LOOK

| Task | Location | Notes |
| --- | --- | --- |
| Shared contract | `crates/contract/src/` | ABI-v1 metadata/codecs and vocabulary; no backing memory, runtime, handler traits, or device execution. |
| Reusable schemas | `crates/hardware-interface-schema/` | Runtime-neutral record/register metadata, scalar codecs, exact-start lookup, format-neutral exports, and default-off generic C11 fragments. |
| Reusable runtime | `crates/hardware-interface-runtime/` | Shared memory, raw register access/observation, `SoftwareCpu`/`CpuLocal`, generic interrupt dispatch, and generic trace storage. |
| Device behavior | `crates/device/src/` | Hardware-owned queue/IRQ registers and state, checked mapping metadata over injected memory access, typed reset/control and shared execution/retirement. |
| Host-side model | `crates/host/src/` | CPU-bound `AsyncDriver`, `PdmaRegisterPort<AsyncDeviceAccess>`, PDMA interrupt adapters, and explicit semantic/async compatibility ports. |
| System scenarios | `crates/scenarios/tests/` | Concurrent actors; `message_irq_linear_copy` is the canonical ABI Message-IRQ E2E. |

## CONVENTIONS

- Use native Rust module layout everywhere; `#[path]` and conditional path
  attributes are forbidden. Only repository `retired/` trees are exempt.
  `cargo test -p hardware-interface-schema-macros --test repository_layout`
  enforces the rule across repository Rust sources during workspace tests.
- Tests use Given/When/Then comments and assert observable completions, events, and memory contents.
- `AsyncDevice::new(PdmaMemoryProvisioning, ResourceConfig)` constructs a matched
  core/runtime pair and returns `(AsyncDevice, AsyncDeviceWorker)`; callers
  choose whether to `tokio::spawn(worker.run())`, directly await it, or use another
  executor strategy.
- `DeviceCore` accepts only external-memory read/write capability. Runtime provisioning
  owns address allocation and backing installation. `from_core(core, provisioning)`
  checks that both share one backend; external backends register pre-provisioned mappings.
- Contract composes GlobalRegisters, QueueRegisters and IrqRegisters from generated
  register layouts and response sub-blocks. Hardware owners directly store their generated
  Values; DeviceCore routes only the three blocks. Do not add a flat leaf dispatch table,
  runtime owner tags or a private register storage backend. Addresses derive from the
  composition; preserve semantic behavior rather than historical numeric placements.
- Explicit manual-drive helpers belong only in manual status/trace and step-budget tests;
  there is no public synchronous device facade. Normal system scenarios run the async worker.
- `device::issue` owns resource admission and runnable selection; `device::probe` only
  observes status. Concrete stage edges live in `device::flow` and carry no retry control.
  The composition root selects normal command work or retained retirement service on
  the same RetirementUnit. Frontend FaultCached completes one request; publication is
  retried only by a subsequent request, without fetching or executing again.
- `AsyncDriver` is CPU-bound: setup requires an active matching `SoftwareCpu`, clones share
  one command-ID/slot accounting state, and it alone services IRQs. `ManualDriver` is the
  explicit CPU-unbound observability facade, constructed with `ManualDriver::setup_on` or
  `ManualDriver::setup_with_config_on`; it has no IRQ service. Both public facades forward
  to one private core with clone-shared accounting. Among system scenarios, only the five
  named zero-entry observability tests use manual setup; focused host tests may exercise it.
- Device-local IRQ enable/mask/ack belongs to `pdma-model-device`; generic handler
  registration, CPU execution and dispatch belong to `hardware-interface-runtime`;
  PDMA source/service/trace adaptation belongs to `pdma-model-host`.
- Software handlers register by `IrqVector` with host IRQ delivery before `run`, never
  with the device. Multiple sources may share one vector; each vector has one persistent
  handler entry, and delivery remains serial and non-reentrant.
- Runtime `CpuLocal<T>` is the modeled single-copy mutable software-state capability. Access is
  synchronous and valid only during a poll of the owning CPU's main or handler future;
  do not expose a lock, guard, raw CPU identity, or await-spanning borrow.
- Await-spanning state private to one vector handler belongs in its persistent
  `StatefulIrqHandler`; do not shuttle handler-owned resources through
  `CpuLocal<Option<T>>` on every IRQ turn.
- The private `SoftwareCpuToken` is installed in the single `CURRENT_SOFTWARE_CPU` option
  only around an actual inner main or handler poll. RAII restores any prior token after the
  poll, so a nested CPU context does not erase its enclosing context.
- Before every nested poll of a CPU-bound `DriverPort` create, submit, doorbell,
  completion, or event future, validate the owning active CPU. A pending future resumed
  inactive or on another CPU returns the typed error before another hardware access.
  The canonical register port performs ordered individual accesses: completed memory
  and MMIO writes survive later failure or cancellation. Device hardware never receives
  a Future-cancellation signal. Keep unresolved command results and driver bookkeeping
  in host state; do not refund an accepted submission or lose an already-consumed record.
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
- `PdmaRegisterPort<AsyncDeviceAccess>` is the canonical frontend for new E2E scenarios.
  Its split runtime memory/register access plus PDMA provisioning and host IRQ coordination
  capabilities add no second executor; explicit semantic/async compatibility ports
  share the same device core, execution and retirement behavior.
- IRQ delivery and register-port IRQ commands share one host coordinator. Claim is
  individual command/result/response accesses; cancelled issued claims are recovered
  before another command overwrites the response. Hardware has no cancellation input.
  `AsyncDeviceAccess::new` and `new_at_mmio_base` require an explicit
  `PdmaIrqCoordinator`; independently constructed adapters over the same device must
  receive clones of the same coordinator. Fully recovered notifications belong to host
  delivery state, not to the subsequently overwritten IRQ response registers.
- A successful whole-device reset checked-advances `DeviceEpoch`, preserves committed
  device-owned PDMA mappings plus runtime memory bytes/revision, trace history, resource configuration, and
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

v0.9 is **COMPLETE**: `model/docs/v0_9_model_plan.md` records the historical data ABI and
the exact 66-register, 4 KiB little-endian MMIO registry, endpoint, host adapter, closed
ordered batches, reset/IRQ behavior, and generated C constants. The active successor
replaces those cross-memory/MMIO batches with host-owned sequential accesses; mapped
activation validates already-committed regions. All frontends retain one
execution/retirement core. v0.8 records the still-binding
vector-to-handler, CPU-local, cancel-safe accounting, bootstrap, and main/handler
alternation boundaries; v0.7 is historical.

`hardware-interface-schema` is reusable infrastructure, not a PDMA runtime replacement:
it owns schema metadata and exports, while PDMA continues to own ring publication,
storage/MMIO effects, DMA, reset, IRQ, actor, and executor behavior. The crate is
`publish = false`; its `export-c-header` backend is default-off and enabled explicitly
by the PDMA contract for the existing exact generated data header.

`hardware-interface-runtime` is the active workspace-private generic runtime. It owns
the default shared byte store and backend-neutral read/write capability, raw register
access/observation, software CPU-local state,
generic interrupt registry/dispatcher contracts, and generic trace storage. PDMA owns
mapping policy, register effects, the canonical register port, driver accounting, IRQ
adaptation, actors and execution/retirement behavior.

## DO NOT

- Do not add callable C APIs, Terminus, PCIe/MSI-X physical maps/table/PBA behavior,
  SR-IOV, FLR, per-queue reset, a second executor/driver/E2E, or timing. Do not expand the
  completed MMIO scope or hand-edit the generated ABI data header.
- Do not modify retired sync crates except for deliberate rollback-snapshot maintenance requested by the user.
- Do not add `unsafe`, `unwrap`, `expect`, `panic`, `todo`, or indexing/slicing in production code.
- Do not grow any Rust source beyond 250 pure LOC; split before adding behavior.
- Do not put backing memory in contract; keep DMA-visible data structures separate from storage media.
- Do not make device depend on host or scenarios.
- Do not make the device store handler tables or invoke software callbacks.
- Do not add IRQ service to `ManualDriver` or bypass `AsyncDriver` CPU validation.
- Do not resurrect the retired transport/bus/port stack in production, tests, or active
  guidance; historical v0.9 prose and the explicit negative compile fixture are the only
  retained references.
