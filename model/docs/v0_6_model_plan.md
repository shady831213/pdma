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

## Boundaries

The v0.5 workspace boundaries remain in force:

| Crate | v0.6 responsibility |
| --- | --- |
| `pdma-model-contract` | Shared host/device contract vocabulary only. No backing memory, device behavior, host port, or runtime surface. |
| `pdma-model-device` | Sync hardware-behavior reference model and explicit hardware turns. No dependency on host or scenarios. |
| `pdma-model-host` | Sync driver actor surface, `DevicePort`, local slot accounting, and legacy wrapper APIs. |
| `pdma-model-scenarios` | Sync reference behavior scenarios that interleave software and hardware turns. |
| `pdma-model-device-async` | Tokio-backed async hardware runner and async device handle. No dependency on host or scenarios. |
| `pdma-model-host-async` | Async driver actor surface and async device-port boundary. |
| `pdma-model-scenarios-async` | Async behavior-match scenarios against the sync reference. |

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
- Full workspace format, test, clippy, and doc validation pass.
