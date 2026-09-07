# PROJECT KNOWLEDGE BASE

Generated: 2026-08-31
Baseline commit: pre-v0.8 checkpoint `0c044bd`; v0.8-v0.9 implementation follows
Branch: master
Scope: v0.9 is COMPLETE, including its historical data ABI and exact 4 KiB numeric MMIO
memory-and-bus contract. Preserve its ABI/reset and validated v0.8 IRQ/CPU boundaries.

## OVERVIEW

This repo is currently being advanced through the Rust PDMA conceptual model. Treat white-board architecture docs as background. `model/docs/v0_model_plan.md` records completed v0 behavior, `model/docs/v0_5_model_plan.md` records the completed workspace boundary refactor, `model/docs/v0_6_model_plan.md` records the completed actor/async transition and v0.6.5 active-model cleanup, `model/docs/v0_7_model_plan.md` preserves the initial host-owned IRQ delivery snapshot, and `model/docs/v0_8_model_plan.md` records the completed vector-dispatch and CPU-local ownership work.

## STRUCTURE

```text
pdma/
├── model/docs/v0_model_plan.md    # completed v0 behavior ledger
├── model/docs/v0_5_model_plan.md  # completed v0.5 workspace/boundary plan
├── model/docs/v0_6_model_plan.md  # completed v0.6 actor/async transition and v0.6.5 cleanup
├── model/docs/v0_7_model_plan.md  # historical v0.7 single-handler IRQ snapshot
├── model/docs/v0_8_model_plan.md  # historical vector-dispatch/CPU-local IRQ plan
├── model/docs/v0_9_model_plan.md  # completed v0.9 data ABI and exact 4 KiB MMIO model
└── model/modeling/                # Rust 2024 schema/runtime/PDMA model workspace
```

Ignore generated `model/modeling/target/` content.

## WHERE TO LOOK

| Task | Location | Notes |
| --- | --- | --- |
| Review completed v0.9 | `model/docs/v0_9_model_plan.md` | Historical ABI plus exact MMIO registry/endpoint/adapter/batches, reset epochs, and header policy. |
| Shared host/device contract | `model/modeling/crates/contract/` | IDs, ABI-v1 metadata/codecs, DMA vocabulary, descriptors, records, controls, and IRQ vocabulary. |
| Reusable schema facility | `model/modeling/crates/hardware-interface-schema/` | Runtime-neutral record/register schema metadata, codecs, exact-start lookup, format-neutral exporters, and default-off generic C11 fragments. |
| Reusable runtime | `model/modeling/crates/hardware-interface-runtime/` | Shared memory, raw register access/observation, software CPU-local state, generic interrupt dispatch, and trace storage. |
| Device behavior | `model/modeling/crates/device/` | Queue-port actor, device-owned PDMA mappings over runtime memory, typed reset/control, shared execution/retirement, trace, and PDMA-local IRQ. |
| Host-side model | `model/modeling/crates/host/` | `PdmaRegisterPort<AsyncDeviceAccess>`, driver accounting, PDMA interrupt adapters, and explicit async/sync compatibility ports. |
| System scenarios | `model/modeling/crates/scenarios/` | Host/device/IRQ stories; `message_irq_linear_copy` is the canonical ABI E2E. |
| Validate workspace | `model/modeling/Cargo.toml` | Workspace lint/profile config. |

## CODE MAP

| Area | Location | Role |
| --- | --- | --- |
| Contract ABI | `crates/contract/src/abi/` | Layout-exact records, registers, MMIO, rings, codecs, and errors. |
| Contract semantic | `crates/contract/src/semantic/` | Layout-independent commands, identities, queue controls, results, IRQ, resources, and trace vocabulary. |
| Device engine | `crates/device/src/engine/` | Deterministic behavior that directly executes contract semantic commands; it is behavior, not another data model. |
| Device ABI state | `crates/device/src/abi/` | Exact DMA addressing, ring state and publication, MMIO behavior, and register endpoint. |
| Device adapter | `crates/device/src/adapter/` | Stateless typed conversion from ABI descriptors into contract semantic commands. |
| Device runtime | `crates/device/src/runtime/` | Actor facade, shared orchestration state, reset, worker, and wait mechanics. |
| Host semantic | `crates/host/src/semantic/` | Driver, accounting, semantic port, and transport-independent errors. |
| Host ABI adapter | `crates/host/src/adapter/abi/` | Register port, device access, exact capabilities, and semantic-to-ABI projection. |
| Host runtime | `crates/host/src/runtime/` | CPU-local and interrupt delivery integration. |
| Scenarios | `crates/scenarios/tests/` | System-level behavioral validation. |

## CURRENT STATE

Completed slices: scaffold, typed IDs, checked memory, generic ring, driver setup, device loop, linear copy, strided copy, event ring, queue config, deterministic arbitration, broadcast copy, vector-add, Resource Credit Model, Backpressure/Stall Annotation Layer, Trace Sink, v0 architecture specification draft, v0.6 actor/async transition, v0.6.5 active-model cleanup, v0.8 vector-routed IRQ contexts, and the historical v0.9 descriptor-layout ABI register-contract.

v0.9 is **COMPLETE**: the historical data ABI is paired with one canonical 66-register,
4 KiB little-endian MMIO contract, device endpoint, clone-shared host adapter, and
generated C constants. Raw activation uses exact committed mappings through the shared
prepared core; two closed ordered batches combine DMA preparation with activation or
producer publication. ABI and MMIO retain one execution/retirement core. Whole-device
reset preserves committed DMA mappings/bytes/generation, trace
history, resource configuration, and lifecycle accounting; it clears resettable
queues/results/work/IRQ and advances `DeviceEpoch`, so stale queue, IRQ, and notification
capabilities fail before mutation and require explicit new-epoch bootstrap. The
checked-in `model/modeling/include/pdma_model_abi.h` is generated from Rust metadata;
regenerate with `pdma-abi-header -- --write include/pdma_model_abi.h` and gate with
`--check include/pdma_model_abi.h`, never manual edits.
v0.8 remains mandatory: `AsyncDriver` is CPU-bound and exclusively services IRQs;
`ManualDriver` is CPU-unbound observability only; one private core shares cancel-safe
accounting; the device never stores or invokes handlers. The old sync stack remains
retired under `model/retired/v0_6_sync_reference/`.

The reusable `hardware-interface-schema` crate is extracted infrastructure, not a new
PDMA hardware/runtime boundary: PDMA still owns rings, register storage and MMIO effects,
DMA, reset epochs, IRQ state machines, callbacks, actors, and executor behavior. Its
generic `export-c-header` backend is default-off and explicitly enabled only by the PDMA
contract for the existing generated data header; the crate remains `publish = false`.

The workspace-private `hardware-interface-runtime` crate owns generic shared memory,
raw register access and observation, `SoftwareCpu`/`CpuLocal`, interrupt registry and
dispatcher mechanics, and generic trace storage. PDMA device/host crates retain PDMA
mapping policy, register effects, driver accounting, exact interrupt adaptation, and
the explicit compatibility ports; the canonical E2E uses `PdmaRegisterPort` over
`AsyncDeviceAccess`.

## CONVENTIONS

- Rust module redirection with `#[path]` is forbidden repository-wide outside
  `retired/` trees, including tests, examples, macros and conditional attributes.
  Use native Rust module declarations and matching directory/file names. Do not
  bypass this rule with `include!`, symlinks, or duplicated source trees.
- Keep the model deterministic and step-based; do not add cycle timing.
- Add failing scenario tests first, then minimum model behavior.
- Preserve existing behavior for linear, strided, event, queue-config, arbitration, broadcast, vector-add, resource-credit, stall-annotation, and trace-sink slices.
- For IRQ scenarios, run the device worker, SW main future, and host IRQ delivery future as explicit concurrent actors.
- In every software-bearing system scenario, call `run_main` exactly once with the complete software program. Only a real `Poll::Pending` ends a main CPU segment; immediately-ready awaits do not.
- Keep handler futures atomic across their complete execution and do not promise IRQ priority before host delivery has requested the FIFO CPU gate.
- Register every host handler by `IrqVector` before delivery starts. Multiple sources may share a vector; each vector has one persistent handler entry.
- Keep IRQ-visible mutable software state in `CpuLocal<T>` or another explicitly modeled single-copy transfer. `CpuLocal::with` is synchronous, CPU-bound, and must not expose a guard or borrow across `.await`.
- Keep await-spanning state private to one handler in its persistent `StatefulIrqHandler`; do not shuttle handler-owned resources through `CpuLocal<Option<T>>` on every IRQ turn.
- CPU-bound `AsyncDriver` setup must occur inside an active matching `SoftwareCpu`; it alone services IRQs. `ManualDriver::setup_on` and `ManualDriver::setup_with_config_on` are CPU-unbound observability setup only and expose no IRQ service. Both facades use one private core and clones share one accounting state. System scenarios use manual setup only in the five named zero-entry observability tests; focused host tests may exercise the explicit manual API itself.
- The private `SoftwareCpuToken` occupies one `CURRENT_SOFTWARE_CPU` option only while an inner main or IRQ-handler poll executes. Its RAII guard restores the prior token after that poll, including a temporarily nested CPU context.
- Every nested poll of a CPU-bound create, submit, doorbell, completion, or event port
  future must first validate the owning active CPU. Migration to inactive or wrong-CPU
  execution returns the typed error before the operation's commit point. Create commits
  only on `Ok(_)`, submit/doorbell only on `Ok(())`, record removal only on
  `Ok(Some(_))`, and dropping a pending port future leaves its device state unchanged.
- Keep actor construction/spawn/shutdown/join, trace instrumentation/collection, and final assertions in the scenario harness. The disabled-submission trace test is the sole intentional trace-control interleaving inside main.
- Pure device/manual observability tests have no artificial `run_main`; the validated scenario invariant is 56 total, 51 one-entry software tests, and the same five zero-entry exceptions documented in the architecture spec.
- Do not register or invoke software handlers from the device loop; handlers belong to host IRQ delivery.
- For ABI-v1, keep wire bytes fixed-width little-endian and regenerate the checked-in C11
  data/MMIO header from canonical Rust metadata. Callable C APIs remain deferred.
- Keep Rust files below 250 pure LOC; split by responsibility before growing `desc.rs` or `device.rs`.
- Use English plain short commit messages matching recent history if the user asks to commit.

## ANTI-PATTERNS

- Do not work outside the model workspace unless the user explicitly changes scope.
- Do not stage or document `model/modeling/target/`.
- Treat `docs/white_boards/**` as exploratory background, not active implementation scope.
- Do not add Terminus, numeric PCIe/MSI-X maps or table/PBA behavior, callable C APIs,
  SR-IOV, FLR, per-queue reset, timing, a second executor/driver/E2E, or unsafe layout
  casts. Do not expand the completed exact MMIO scope or hand-edit the generated header.
- Do not add a duplicate host enable/mask layer while the modeled system has only one PDMA device; device-local mask/ack remains distinct from host handler dispatch.
- Do not expose raw CPU identity, mutex guards, async CPU-local borrows, or a manual-driver escape hatch for software-bearing scenarios. Do not give `ManualDriver` IRQ service or weaken `AsyncDriver` CPU validation.
- Do not publish success completions after memory faults.
- Do not weaken tests to pass validation.

## COMMANDS

```bash
cd model/modeling
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps --document-private-items
cargo run -p pdma-model-contract --bin pdma-abi-header -- --check include/pdma_model_abi.h
```

Use `GIT_MASTER=1` before every git command.

## NOTES

For follow-on work, keep the active successor boundaries aligned with
`model/docs/pdma_arch_spec_v0.md`: contract has vocabulary only; runtime owns generic
CPU-local and interrupt-dispatch mechanics; device has no host dependency and never
invokes handlers; host owns PDMA driver/register/interrupt adaptation; and scenarios
wire the actors together.
