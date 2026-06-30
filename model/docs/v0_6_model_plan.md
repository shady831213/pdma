# PDMA v0.6 Model Plan

## Purpose

v0.6 changes the executable model from a purely call-driven sync shape toward an
actor-style host/device interaction model. The sync model remains the reference path in
phase 1. A later phase may add parallel async crates that must match the sync-observed
behavior before any replacement is considered.

## Phase 1: Sync Actor Reference

Phase 1 keeps the current four-crate workspace and adds an actor-style software boundary
without adding an async runtime. Host software acts through a synchronous `DevicePort`,
while hardware still advances through explicit device turns such as `step()`,
`step_with_status()`, `run_until_idle()`, and `run_until_blocked()`.

The goal is to make scenario code express software and hardware turns separately:

```text
software submits through DevicePort
software may poll before hardware progresses
hardware takes one or more explicit turns
software observes completions or events
```

Phase 1 owns these changes:

- Add a sync `DevicePort` boundary in the host crate.
- Add `Driver::*_on` methods that use the port instead of hard-coding the concrete device
  type in the actor-facing API.
- Keep existing `Driver::*` methods as sync convenience wrappers for the current model.
- Add scenario coverage for software/hardware interleaving and completion backpressure.

Phase 1 must not add Tokio, futures, async-std, smol, channels, or runtime dependencies.
It also must not introduce descriptor byte layout, register offsets, C ABI, generated
headers, interrupt/reset modeling, IOMMU, cache coherency, or real OS integration.

## Phase 2: Parallel Async Crates

Phase 2 is intentionally separate from phase 1. It may add parallel async crates such as:

```text
pdma-model-host-async
pdma-model-device-async
pdma-model-scenarios-async
```

Those crates should depend on the same shared contract vocabulary and should behavior-match
the sync reference scenarios. The difference should be execution shape only: software and
hardware become futures that can be driven by `join!`, `block_on`, or spawned on a runtime.

Async crates must not force async or runtime dependencies into the existing sync crates.
Tokio is a reasonable default runtime candidate for phase 2, but Tokio is not part of phase
1. If the async version proves stable and covers the sync reference behavior, a later phase
can decide whether to retire the sync implementation.

## Boundaries

The v0.5 workspace boundaries remain in force:

| Crate | v0.6 responsibility |
| --- | --- |
| `pdma-model-contract` | Shared host/device contract vocabulary only. No backing memory, device behavior, host port, or runtime surface. |
| `pdma-model-device` | Sync hardware-behavior reference model and explicit hardware turns. No dependency on host or scenarios. |
| `pdma-model-host` | Sync driver actor surface, `DevicePort`, local slot accounting, and legacy wrapper APIs. |
| `pdma-model-scenarios` | Sync reference behavior scenarios that interleave software and hardware turns. |

## Completion Criteria

- Actor-style scenarios use `Driver::*_on` and explicit device turns.
- A software poll before hardware progress observes no completion.
- Completion publication pressure requires a software polling turn before hardware retires
  the next command.
- Existing sync driver APIs remain available and behavior-preserving.
- The sync workspace has no async/runtime dependency.
- Full workspace format, test, clippy, and doc validation pass.
