# PDMA v0.7 Model Plan

> Historical snapshot: this file records the completed v0.7 checkpoint. Its
> single-handler wording and 52/47/5 test counts are intentionally retained as v0.7
> closeout facts. v0.8 supersedes the active host-delivery shape with a fixed
> vector-to-handler registry, CPU-local state, and 54/49/5 scenario counts; see
> `model/docs/v0_8_model_plan.md`.

## Purpose

v0.7 adds a complete conceptual interrupt model at the current queue-port layer. It
models three concurrent actors:

```text
PDMA device actor
  owns DMA execution, completion/event publication, and the PDMA-local IRQ controller

Host IRQ delivery actor
  adapts device notifications into one registered software handler

SW main future
  is one complete program owning driver setup, submission, polling, and IRQ configuration
```

The goal is to model software main-context and IRQ-context interaction without copying
`etha_model`'s unsafe boundary where a device loop directly invokes software handlers.
The device may assert interrupt state, but software handlers are dispatched only by a
host-owned IRQ delivery actor.

## Non-Goals

v0.7 does not add exact descriptor byte layout, register offsets, generated C headers,
PCIe MSI/MSI-X address/data encoding, MSI-X tables or PBA state, APIC/GIC details, CPU
trap frames, privilege transitions, IOMMU, cache coherency, cycle timing, or real OS
integration. The long-term bus boundary remains deferred; v0.7 keeps the queue-port API
while modeling both stateful line delivery and discrete message delivery.

## Crate Boundaries

| Crate | v0.7 responsibility |
| --- | --- |
| `pdma-model-contract` | Shared IRQ vocabulary: typed vectors, lines, sources, routes, snapshots, messages, and notifications. No runtime, storage, handler traits, or device behavior. |
| `pdma-model-device` | PDMA device actor plus PDMA-local IRQ controller. It records cause/status, source enable/mask, ack/clear, line assertion, and discrete messages. It never stores or invokes software handlers. |
| `pdma-model-host` | Host execution environment plus PDMA driver. It owns source demultiplexing, the single CPU token, handler registration, and notification delivery. |
| `pdma-model-scenarios` | End-to-end stories that run the device worker, SW main future, and IRQ delivery future concurrently. |

## Host Module Roles

The host crate is no longer just a driver wrapper. It models the host-side execution
environment needed to deliver device interrupts into software contexts.

```text
crates/host/src/
  driver/          normal PDMA driver path and IRQ handler body helpers
  port/            driver-visible device access boundary
  irq_delivery/    host IRQ delivery actor, handle, and handler lifecycle trace
  context/         CPU token shared by SW main and IRQ handler contexts
```

The first implementation may keep modules smaller than this sketch, but responsibilities
must stay separated. Delivery logic must not be folded into `driver`, and handler tables
must not be stored in `device`.

## Interrupt Layers

v0.7 keeps device interrupt state separate from host software dispatch. Because the
modeled system contains only one PDMA device, the host adapter deliberately has no
second enable/mask layer.

### PDMA-Local Interrupt Controller

The device-local controller belongs to the PDMA device model. It owns:

- source cause/status bits;
- source enable state;
- source mask state;
- explicit device ack/clear semantics;
- stateful IRQ line assertion or discrete message emission.

Completion and event publications latch device IRQ causes only after the record is
actually published. A stalled completion/event publication must not assert an IRQ until
the retry succeeds.

### Host IRQ Delivery Actor

At the v0.7 checkpoint, the host delivery actor adapted the device notification
boundary into software context and owned:

- type-state registration of one PDMA handler;
- serial notification delivery;
- acquisition of the shared CPU token for the complete handler future;
- handler start/completion trace events;
- the delivery future run loop and final successful-delivery summary.

Handlers register with the delivery actor, not with the device. The delivery actor may
call driver code through host-facing APIs; it never receives direct mutable access to
device internals.

System enable/disable, system mask/unmask, and a separate system completion operation
are intentionally omitted. They would duplicate device-local controls without adapting
or isolating another device in the current one-device system.

## Required Semantics

- Device-local mask suppresses line assertion while preserving pending device causes.
- Device ack clears a selected Line cause only when its completion/event path is empty;
  a record published after the handler's final empty poll keeps the cause pending and
  asserted. Ack recomputes line assertion in either case.
- `Line` delivery is a stateful asserted/deasserted observation.
- `Message` delivery is a discrete transaction carrying a vector and per-source
  monotonic message identity. Every trigger within representable model state is
  preserved; v0.7 adds no intentional coalescing or moderation.
- Message delivery and device cause clear are distinct operations. Redundant messages
  are handled in software by idempotently reading/draining status and acking the source.
- One IRQ delivery actor runs at most one handler future at a time.
- IRQs that arrive while a handler is running remain pending and dispatch only after the
  current handler returns.
- `SoftwareCpu::run_main` receives one complete main future. One main atomic segment is
  exactly one poll ending in `Poll::Ready` or `Poll::Pending`; only a real pending result
  releases the CPU before the next main poll. Immediately-ready `.await` chains remain
  in the same segment.
- A handler may be expressed as an async Rust future, but it owns the modeled CPU token
  for the entire future, including suspension. SW main cannot execute during that
  interval, and nested IRQ handlers are unsupported.
- CPU request order is FIFO. IRQ precedence over main begins only when the delivery actor
  has observed the notification and requests the CPU before main's next reacquisition.
  An unobserved notification has no CPU priority, a later IRQ request cannot overtake an
  already queued main request, and v0.7 makes no global scheduler-priority promise.
- The device actor remains independent and may progress while software is in IRQ context.
- Host shutdown has deterministic priority when shutdown and a notification are ready
  together. Successful handler turns are counted in the actor's final summary; scenarios
  use handler channels when they need intermediate synchronization.

## Implementation Slices

### Slice 1: Documentation and IRQ Vocabulary — Complete

- Refresh stale root and workspace `AGENTS.md` files for the v0.7 state.
- Add shared IRQ contract types and tests.
- Keep the types runtime-free and device/host neutral.

### Slice 2: PDMA-Local IRQ Controller — Complete

- Add a device-local IRQ controller under the queue-port device model.
- Add unit or public-surface tests for source enable, source mask, pending latch, line
  assertion, ack clearing, and masked-pending preservation.
- Keep `SyncDevice` and `AsyncDeviceHandle` behavior equivalent where the same surface
  exists.

### Slice 3: Completion/Event IRQ Latching — Complete

- Latch completion IRQ causes after successful completion publication.
- Latch event IRQ causes after successful event publication.
- Verify publication pressure delays IRQ assertion until the record is actually
  published.

### Slice 4: Device IRQ APIs — Complete

- Expose device-local IRQ status, enable, mask, unmask, ack, and wait-line APIs through
  async and manual device surfaces.
- Extend the host port boundary without making host logic depend on device internals.

### Slice 5: Host Module Reorganization — Complete

- Split host roles so `driver`, `port`, and `irq_delivery` are distinct modules.
- Preserve existing `AsyncDriver` and `AsyncDevicePort` public use paths unless an
  intentional re-export keeps downstream tests stable.

### Slice 6: IRQ Delivery Actor — Complete

- Add a host-owned delivery actor and handle.
- Support type-state handler registration, explicit shutdown, and final summary
  accounting without a separate delivery-progress watch.
- Keep caller-owned spawn policy: scenarios decide whether to `tokio::spawn`, `join!`,
  or directly await the delivery future.

### Slice 7: Handler Atomicity — Complete

- Enforce one modeled CPU token across SW main and IRQ handler contexts.
- Run one complete main future as poll-to-pending atomic segments while retaining FIFO
  acquisition state across outer polls.
- Test that an IRQ arriving during a handler remains pending and dispatches only after
  the current handler future completes.

### Slice 8: Main Context and IRQ Context Scenarios — Complete

- Add scenarios where the SW main future submits work and does not inline-poll the
  completion/event.
- Keep setup, configuration, submission, and software waits inside that single complete
  main entry rather than manually invoking one turn at a time.
- The IRQ delivery actor dispatches the handler; the handler drains completion/event and
  acks device causes.
- The main future observes handler completion through explicit synchronization.

### Slice 9: Line/Message and Idempotence Matrix — Complete

- Prove masked device causes/messages remain pending until unmask.
- Prove line ack deasserts stateful delivery.
- Prove a Line cause relatched between the final empty poll and ack remains asserted.
- Prove message triggers are not coalesced and a forced redundant empty delivery is
  software-safe.

### Slice 10: IRQ Trace and Documentation Closeout — Complete

- Add trace events for IRQ source latched, line asserted/deasserted, message emitted,
  device ack, handler started, and handler completed.
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
  message, and ack semantics.
- Completion and event publication assert IRQ only after successful publication.
- Host has clear `driver`, `port`, and `irq_delivery` roles.
- Handlers register with host IRQ delivery, not device.
- The IRQ delivery actor guarantees non-reentrant handler execution.
- Scenarios prove one complete SW main and IRQ contexts serialize at real main
  suspension boundaries while the device actor remains independent.
- Scenarios prove line completion and message event delivery through three actors.
- Message tests prove discrete delivery without coalescing and a deterministic empty
  redundant handler turn.
- Line tests prove ack cannot clear a relatched cause while its record remains pending.
- Host tests prove deterministic shutdown priority and final successful-delivery
  accounting, including that handler errors are not counted.
- At v0.7 closeout, across all 52 scenario tests, 47 software-bearing tests used exactly
  one complete `run_main`; five pure device/manual observability tests used none.
- Existing queue-port behavior remains green.
