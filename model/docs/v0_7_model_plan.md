# PDMA v0.7 Model Plan

## Purpose

v0.7 adds a complete conceptual interrupt model at the current queue-port layer. It
models three concurrent actors:

```text
PDMA device actor
  owns DMA execution, completion/event publication, and the PDMA-local IRQ controller

Host IRQ delivery actor
  owns system-level IRQ dispatch, handler registration, and handler atomicity

SW main future
  owns normal driver setup, submission, polling, and device IRQ configuration
```

The goal is to model software main-context and IRQ-context interaction without copying
`etha_model`'s unsafe boundary where a device loop directly invokes software handlers.
The device may assert interrupt state, but software handlers are dispatched only by a
host-owned IRQ delivery actor.

## Non-Goals

v0.7 does not add exact descriptor byte layout, register offsets, generated C headers,
MSI/MSI-X vectors, APIC/GIC details, CPU trap frames, privilege transitions, IOMMU,
cache coherency, cycle timing, or real OS integration. The long-term bus boundary
remains deferred; v0.7 keeps the queue-port API while making interrupt behavior
observable and concurrent.

## Crate Boundaries

| Crate | v0.7 responsibility |
| --- | --- |
| `pdma-model-contract` | Shared IRQ vocabulary: typed vectors, sources, causes, snapshots, and config values. No runtime, storage, handler traits, or device behavior. |
| `pdma-model-device` | PDMA device actor plus PDMA-local IRQ controller. It records cause/status, source enable/mask, ack/clear, and line assertion. It never stores or invokes software handlers. |
| `pdma-model-host` | Host-side execution environment plus PDMA driver. It owns `driver`, `port`, and `irq_delivery` roles, including handler registration and system dispatch. |
| `pdma-model-scenarios` | End-to-end stories that run the device worker, SW main future, and IRQ delivery future concurrently. |

## Host Module Roles

The host crate is no longer just a driver wrapper. It models the host-side execution
environment needed to deliver device interrupts into software contexts.

```text
crates/host/src/
  driver/          normal PDMA driver path and IRQ handler body helpers
  port/            driver-visible device access boundary
  irq_delivery/    system IRQ delivery actor, handle, registry, and vector mask state
  context/         main-context and IRQ-context values when scenarios need them
```

The first implementation may keep modules smaller than this sketch, but responsibilities
must stay separated. Delivery logic must not be folded into `driver`, and handler tables
must not be stored in `device`.

## Interrupt Layers

v0.7 deliberately keeps two interrupt-control layers distinct.

### PDMA-Local Interrupt Controller

The device-local controller belongs to the PDMA device model. It owns:

- source cause/status bits;
- source enable state;
- source mask state;
- explicit device ack/clear semantics;
- device IRQ line assertion.

Completion and event publications latch device IRQ causes only after the record is
actually published. A stalled completion/event publication must not assert an IRQ until
the retry succeeds.

### System IRQ Delivery Actor

The host delivery actor models the hardware/software boundary after a device line is
observable by the host system. It owns:

- handler registration;
- system delivery mask/unmask state;
- in-service state;
- serial handler dispatch;
- the delivery future run loop.

Handlers register with the delivery actor, not with the device. The delivery actor may
call driver code through host-facing APIs; it never receives direct mutable access to
device internals.

## Required Semantics

- Device-local mask suppresses line assertion while preserving pending device causes.
- System delivery mask suppresses handler dispatch while preserving device pending work.
- Device ack clears selected device pending causes and recomputes line assertion.
- System completion/ack clears only delivery in-service bookkeeping.
- One IRQ delivery actor runs at most one handler future at a time.
- IRQs that arrive while a handler is running remain pending and dispatch only after the
  current handler returns.
- A handler may be async; atomicity means non-reentrant delivery, not executor-level
  non-yielding execution.
- The SW main future may run concurrently with IRQ context and must coordinate shared
  software resources through explicit safe abstractions.

## Implementation Slices

### Slice 1: Documentation and IRQ Vocabulary

- Refresh stale root and workspace `AGENTS.md` files for the v0.7 state.
- Add shared IRQ contract types and tests.
- Keep the types runtime-free and device/host neutral.

### Slice 2: PDMA-Local IRQ Controller

- Add a device-local IRQ controller under the queue-port device model.
- Add unit or public-surface tests for source enable, source mask, pending latch, line
  assertion, ack clearing, and masked-pending preservation.
- Keep `SyncDevice` and `AsyncDeviceHandle` behavior equivalent where the same surface
  exists.

### Slice 3: Completion/Event IRQ Latching

- Latch completion IRQ causes after successful completion publication.
- Latch event IRQ causes after successful event publication.
- Verify publication pressure delays IRQ assertion until the record is actually
  published.

### Slice 4: Device IRQ APIs

- Expose device-local IRQ status, enable, mask, unmask, ack, and wait-line APIs through
  async and manual device surfaces.
- Extend the host port boundary without making host logic depend on device internals.

### Slice 5: Host Module Reorganization

- Split host roles so `driver`, `port`, and `irq_delivery` are distinct modules.
- Preserve existing `AsyncDriver` and `AsyncDevicePort` public use paths unless an
  intentional re-export keeps downstream tests stable.

### Slice 6: IRQ Delivery Actor

- Add a host-owned delivery actor and handle.
- Support handler registration and system mask/unmask.
- Keep caller-owned spawn policy: scenarios decide whether to `tokio::spawn`, `join!`,
  or directly await the delivery future.

### Slice 7: Handler Atomicity

- Enforce globally non-reentrant dispatch in v0.7.
- Test that an IRQ arriving during a handler remains pending and dispatches only after
  the current handler future completes.

### Slice 8: Main Context and IRQ Context Scenarios

- Add scenarios where the SW main future submits work and does not inline-poll the
  completion/event.
- The IRQ delivery actor dispatches the handler; the handler drains completion/event and
  acks device causes.
- The main future observes handler completion through explicit synchronization.

### Slice 9: Mask/Ack Semantic Matrix

- Prove device mask differs from system mask.
- Prove device ack differs from system delivery completion.
- Prove enable/disable differs from mask/unmask.

### Slice 10: IRQ Trace and Documentation Closeout

- Add trace events for IRQ source latched, line asserted/deasserted, device ack,
  delivery masked, handler started, and handler completed.
- Update architecture docs to mark v0.7 IRQ behavior as implemented while keeping
  bus/MMIO/MSI-X details deferred.

## Validation

Run these gates from `model/modeling` after each substantial slice and before final
handoff:

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps --document-private-items
```

If LSP diagnostics are unavailable, record that explicitly and rely on the Rust gates
above.

## Completion Criteria

- Shared IRQ vocabulary is public and tested.
- The PDMA device has a local IRQ controller with source enable, mask, pending, line,
  and ack semantics.
- Completion and event publication assert IRQ only after successful publication.
- Host has clear `driver`, `port`, and `irq_delivery` roles.
- Handlers register with host IRQ delivery, not device.
- The IRQ delivery actor guarantees non-reentrant handler execution.
- Scenarios prove concurrent SW main and IRQ context behavior.
- Scenarios prove device-local mask/ack differs from system delivery mask/completion.
- Existing queue-port behavior remains green.
