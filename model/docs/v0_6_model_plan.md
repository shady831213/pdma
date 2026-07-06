# PDMA v0.6 Model Plan

## Purpose

v0.6 refactors the executable model toward an actor-style host/device interaction model. The sync implementation is the reference path. Async crates now advance in Phase 2 after the sync actor-style reference was locked.

## Phase 1: Sync Actor Reference

Phase 1 keeps the existing four-crate workspace and adds an actor-style software boundary. It adds no async runtime. Host software acts through a synchronous `DevicePort`. Hardware runs in a dedicated `std::thread` forever loop with explicit shutdown, `join`, and error reporting.

Phase 1 is split into three slices.

### Phase 1A: DevicePort and Driver::*_on

Add the sync software boundary and port-driven driver methods:

- Add a sync `DevicePort` boundary in the host crate.
- Add `Driver::*_on` methods that take the port instead of hard-coding the concrete device type in the actor-facing API.
- Keep existing `Driver::*` methods as sync convenience wrappers for the current model.

### Phase 1B: Sync Forever-Loop Hardware Runner

Replace explicit per-scenario device turns with a background hardware runner that owns the device turn loop:

- The runner lives in a `std::thread` forever loop.
- The loop advances the device model and waits on a Condvar/channel/wake for progress or shutdown.
- Shutdown is explicit: a stop signal breaks the loop, the thread joins, and errors are reported.
- Tests wait on Condvar/channel/wake/progress signals; they must not use sleeps.

### Phase 1C: Scenario Migration

Migrate scenarios to the actor-style shape:

- Scenarios submit through `DevicePort` and `Driver::*_on`.
- Primary actor-reference scenarios wait on runner progress/error signals; direct `step()`, `run_until_idle()`, or `run_until_blocked()` calls remain only for explicit helper/status/trace/arbitration and step-budget behavior.
- Add coverage for software/hardware interleaving, polling before completion, and completion backpressure.

Phase 1 must not add Tokio, futures, async-std, smol, or any async runtime dependency to the existing sync crates. It also must not introduce descriptor byte layout, register offsets, C ABI, generated headers, interrupt/reset modeling, IOMMU, cache coherency, or real OS integration.

## Phase 2: Parallel Async Crates

Phase 2 started after Phase 1 completed and the sync reference behavior was locked. It adds parallel async crates such as:

```text
pdma-model-host-async
pdma-model-device-async
pdma-model-scenarios-async
```

Those crates depend on the same shared contract vocabulary and behavior-match the sync reference scenarios incrementally. The difference should be execution shape only: software and hardware become futures that can be driven by `join!`, `block_on`, or spawned on a runtime.

Async crates must not force async or runtime dependencies into the existing sync crates. Tokio is the Phase 2 runtime for async crates only. The first async slice added the async workspace members, async hardware runner, async host linear-copy path, and async linear-copy E2E scenario. If the async version proves stable and covers the sync reference behavior, a later phase can decide whether to retire the sync implementation.

### Phase 2 Architecture Correction: Async-Native Hardware

The async crates are not intended to remain a Tokio wrapper around the sync `PdmaDevice` implementation. The sync crates stay as the deterministic reference, but async hardware behavior should become async-native and independent from the sync device crate.

Target dependency shape:

```text
pdma-model-contract
  shared IDs, descriptors, records, queue/resource configuration, and vocabulary

pdma-model-device
  sync deterministic hardware reference, explicit step/status APIs, sync runner

pdma-model-device-async
  async-native hardware actor/model, async queues, async backpressure, async events

pdma-model-host
  sync imperative driver over the sync DevicePort boundary

pdma-model-host-async
  imperative async driver client over AsyncDevicePort

pdma-model-scenarios
  sync reference scenarios

pdma-model-scenarios-async
  async system scenarios
```

`pdma-model-device-async` must not depend on `pdma-model-device` once the async-native rewrite begins. If sync and async implementations need shared pure logic, that logic should move to a neutral crate or remain in the contract crate only when it is truly shared vocabulary rather than backing-store, runner, or execution behavior.

The async hardware model should own the async execution boundary. Stalls, backpressure, completion/event publication, interrupt-like notification, and eventual lower-level bus actions should be expressed as async hardware behavior rather than collapsed into a sync `step_with_status()` call behind a Tokio mutex.

### Phase 2 Software Boundary

Async software is still required, but it should remain an imperative driver client. Its `await` points belong at the hardware boundary, not in local bookkeeping.

At the current conceptual queue-port layer, valid async boundary operations include queue creation, buffer access, command submission, doorbell ringing, completion polling, and event polling. Local command ID allocation, descriptor construction, queue-handle ownership, and software slot accounting remain ordinary command-style driver logic.

In a later lower-level model, these await points should move down from semantic driver operations to bus-facing actions such as register reads/writes, DMA-visible memory reads/writes, interrupt waits, and interrupt acknowledgement. Higher-level driver operations like queue creation and command submission should then be implemented as imperative sequences composed from those lower-level async bus actions.

### Phase 2 Scenario Shape

Async scenarios are naturally async system stories. They should drive host software through awaitable hardware boundaries and observe asynchronous hardware outcomes. The scenarios should not merely wrap sync scenario logic in `await`; they should express the intended host/device interleaving at the active modeling layer.

Until the async-native hardware split is complete, existing async scenario coverage documents behavior parity of the current transitional async facade. New Phase 2 work should prioritize replacing that facade with an async-native device implementation before adding more parity tests that depend on sync device internals.

### Phase 2 Queue-Port Parity Audit

As of `82a195e Add async manual blocked parity`, the current queue-port-layer async
surface has no known smallest remaining sync-reference behavior gap worth implementing
as another behavior slice. The async suite covers the sync reference behavior across
matching async-native runner, manual-device, trace, resource, and publication-pressure
tests rather than by requiring one-to-one test file names.

Current sync-to-async coverage map:

| Sync reference | Async coverage | Status |
| --- | --- | --- |
| `crates/scenarios/tests/sync_actor.rs` | `crates/scenarios-async/tests/runner_surface.rs`, `crates/scenarios-async/tests/resource_publication_pressure.rs` | Covered. The completion-publication retry behavior is validated by the publication-pressure tests instead of duplicating the sync runner-surface story. |
| `crates/scenarios/tests/resource_credits.rs` | `crates/scenarios-async/tests/resource_credits.rs`, `crates/scenarios-async/tests/resource_publication_pressure.rs` | Covered. Resource stalls and completion/event publication ordering are split by concern in async tests. |
| `crates/scenarios/tests/trace_sink.rs` | `crates/scenarios-async/tests/trace_sink.rs`, `crates/scenarios-async/tests/trace_sink_manual.rs`, `crates/scenarios-async/tests/trace_sink_parity.rs` | Covered. Lifecycle, JSON export, status tracing, manual `run_until_idle`, and ordering parity are represented. |
| `crates/scenarios/tests/trace_sink_polling.rs` | `crates/scenarios-async/tests/trace_sink_edges.rs`, `crates/scenarios-async/tests/error_event.rs` | Covered. Async trace polling records the trace edge, while async error-event tests cover unrelated-queue event polling behavior. |
| `crates/scenarios/tests/stall_annotations.rs` | `crates/scenarios-async/tests/stall_annotations.rs`, `crates/scenarios-async/tests/trace_sink_parity.rs` | Covered. Runner status waits, manual `run_until_blocked`, and traced stall ordering are represented. |

Validation baseline for this audit:

```bash
cargo test -p pdma-model-scenarios --test sync_actor --test resource_credits --test trace_sink --test trace_sink_polling --test stall_annotations
cargo test -p pdma-model-scenarios-async --test runner_surface --test resource_credits --test resource_publication_pressure --test trace_sink --test trace_sink_edges --test trace_sink_manual --test trace_sink_parity --test stall_annotations
```

The bus/MMIO/DMA-memory/IRQ boundary described below remains a long-term direction,
not a current v0.6 parity gap. It is also outside the current sync reference, so it
must not be counted as async work required to match v0.6 queue-port behavior.

### Long-Term Async Boundary Goal

The conceptual async queue-port boundary is an intermediate modeling layer. The long-term direction is a bus-level async boundary where the host driver performs awaitable register, DMA-memory, and interrupt operations, and higher-level driver methods are imperative sequences built from those operations.

Long-term boundary examples:

- `reg_write(offset, value).await`
- `reg_read(offset).await`
- `mem_write(range, bytes).await`
- `mem_read(range).await`
- `wait_irq().await`
- `ack_irq(vector_or_status).await`

At that level, operations such as queue creation, command submission, completion polling, and interrupt handling are no longer primitive port calls. They become driver procedures that prepare DMA-visible memory, program MMIO registers, ring doorbells, wait for interrupts, inspect status, read completion memory, and acknowledge interrupts.

This is the point where device interrupts become naturally observable. A high-level `submit_copy().await` collapses descriptor writes, doorbell writes, hardware execution, completion publication, and interrupt assertion into one semantic operation. A bus-level boundary leaves space for the device actor to assert an interrupt between host actions, and for async scenarios to verify the host/device interleaving explicitly.

## Boundaries

The v0.5 workspace boundaries remain in force:

| Crate | v0.6 responsibility |
| --- | --- |
| `pdma-model-contract` | Shared host/device contract vocabulary only. No backing memory, device behavior, host port, or runtime surface. |
| `pdma-model-device` | Sync hardware-behavior reference model and explicit hardware turns. No dependency on host or scenarios. |
| `pdma-model-host` | Sync driver actor surface, `DevicePort`, local slot accounting, and legacy wrapper APIs. |
| `pdma-model-scenarios` | Sync reference behavior scenarios that interleave software and hardware turns. |
| `pdma-model-device-async` | Async-native hardware actor/model and async device handle. No dependency on sync device, host, or scenarios. |
| `pdma-model-host-async` | Imperative async driver client and async device-port boundary. No dependency on sync host. |
| `pdma-model-scenarios-async` | Async system scenarios against the async-native hardware model and sync reference behavior. |

## Completion Criteria

- Phase 1A: `DevicePort` exists and `Driver::*_on` methods use it.
- Phase 1B: A `std::thread` forever-loop hardware runner runs the device model, with explicit shutdown, join, and error reporting.
- Phase 1C: Primary actor-style scenarios use `Driver::*_on` plus runner progress/error waits instead of explicit device turns.
- Direct device-turn tests are limited to helper/status/trace/arbitration and step-budget behavior, and each remaining call site is intentional.
- A software poll before hardware progress observes no completion.
- Completion publication pressure requires a software polling turn before hardware retires the next command.
- Existing sync driver APIs remain available and behavior-preserving.
- The sync workspace has no async/runtime dependency.
- Phase 2 first slice: async crates exist, async linear-copy behavior matches the sync reference, and Tokio stays confined to async crates.
- Phase 2 architecture correction: async device behavior is async-native and no longer depends on the sync `pdma-model-device` implementation.
- Phase 2 software boundary: host async APIs await only hardware-boundary operations while local driver bookkeeping remains imperative.
- Phase 2 lower-level direction: future MMIO, DMA-memory, and interrupt modeling moves await points down to register, memory, and IRQ actions.
- Full workspace format, test, clippy, and doc validation pass.
