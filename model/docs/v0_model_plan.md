# PDMA v0 Model Progress Plan

## Purpose

This file tracks executable v0 model progress. White-board documents explain architecture direction; this plan records what the Rust model has already proven, what is active next, and what must stay deferred until the current mechanisms are validated.

## Current Ground Rules

- Keep v0 deterministic and step-based with `step()` and `run_until_idle()` scenario tests.
- Run the active device, SW main, and IRQ delivery actors on Tokio while keeping device
  behavior deterministic and step-based.
- Represent each modeled software program as one complete `run_main` future. A main
  atomic segment is one poll, and only a real pending result admits IRQ execution before
  the next main poll; a complete handler future remains atomic and non-nested.
- Prefer native behavioral operation derivation over cycle-accurate timing.
- Keep byte-accurate external descriptor layout, register offsets, C ABI, and generated headers abstract until the model behavior stabilizes.
- Validate every model slice with `cargo fmt --check`, `cargo test`, `cargo clippy --all-targets -- -D warnings`, and `cargo doc --no-deps --document-private-items`.

## Progress Ledger

| Slice | Status | Evidence | Notes |
| --- | --- | --- | --- |
| Crate scaffold | Done | `model/modeling/Cargo.toml`, strict lint/format config | Rust 2024 library crate is in place. |
| Typed IDs | Done | `ids.rs` | `QueueId`, `CommandId`, `BufferId`, and `CompletionRingId`. |
| Checked memory | Done | `memory.rs` unit tests | Bounds and range-overflow errors are typed. |
| Generic ring | Done | `rings.rs` unit tests | Full, empty, wrap, and reuse behavior covered. |
| Driver setup and linear submit/poll | Done | `driver.rs`, `linear_copy.rs` | Completion-based local command slot reclaim. |
| Device step loop | Done | `crates/device/src/runner/native/step.rs`, `tests/linear_copy.rs` | `step()` executes at most one command; `run_until_idle()` reports step budget exhaustion. |
| Linear copy lowering | Done | `crates/device/src/runner/native/copy.rs`, `tests/linear_copy.rs` | One external copy executes as one native range transfer and publishes one completion. |
| 2D strided copy lowering | Done | `crates/device/src/runner/native/copy.rs`, `tests/strided_copy.rs` | One external strided command derives row chunks and one completion; checked memory errors do not publish success. |
| Event ring and recoverable error reporting | Done | `event.rs`, `error_event.rs` | Memory faults publish software-visible events, skip success completions, and keep later queued commands runnable. |
| Queue directory and explicit queue configuration objects | Done | `queue.rs`, `queue_config.rs` | Queue creation stores directory entries, driver owns config handles, and queue-local event polling preserves ownership. |
| Multi-queue arbitration with deterministic policy | Done | `arbitration.rs`, `multi_queue.rs` | Default round-robin policy selects oldest runnable queues and prevents equal-priority queue starvation. |
| Broadcast copy lowering | Done | `crates/device/src/runner/native/broadcast.rs`, `tests/broadcast_copy.rs` | One command fans one source range out to multiple destination ranges with one completion or one memory-fault event. |
| Vector-add operation | Done | `crates/device/src/runner/native/vector_add.rs`, `tests/vector_add.rs` | One command reads two source ranges, performs byte-wise wrapping addition, writes one destination range, and preserves completion/event behavior. |
| Resource credit model | Done | `crates/device/src/runner/native/resources.rs`, `crates/device/src/runner/native/retirement.rs`, `tests/resource_credits.rs` | Internal descriptor, read, write, compute, completion, and event publication pressure can stall deterministic progress without losing queued work or retirement order. |
| Backpressure/Stall Annotation Layer | Done | `crates/device/src/runner/native/step.rs`, `crates/device/src/runner/native/retirement.rs`, `tests/stall_annotations.rs` | Scenario-visible stall reasons distinguish idle, resource pressure, completion pressure, event pressure, and fault-related retirement behavior. |
| Trace Sink | Done | `crates/device/src/trace/`, native runner hooks, `crates/device/src/lib.rs` re-exports, `tests/trace_sink.rs` | In-memory deterministic trace events and Chrome/Perfetto JSON export via sequence ordinals; public API `enable_tracing`, `disable_tracing`, `clear_trace`, `trace_events`, `trace_json`. |
| v0 architecture specification draft | Done | `model/docs/pdma_arch_spec_v0.md` | The original v0 behavior was documented with ABI, register, cycle timing, interrupt, and reset details deferred at that milestone. |
| v0.7 host-owned IRQ delivery | Done | `model/docs/v0_7_model_plan.md`, `crates/scenarios/tests/irq_delivery.rs` | Line and Message routes, device-local IRQ state, one complete software main, poll-boundary CPU serialization, whole-handler atomicity, driver drain/ack, final delivery summaries, and IRQ trace are validated; concrete MSI/MSI-X transport remains deferred. |
| v0.8 vector-routed IRQ contexts | Done | `model/docs/v0_8_model_plan.md`, host/context/driver/IRQ tests, `crates/scenarios/tests/irq_delivery/` | Line and Message notifications dispatch through a fixed per-vector host registry; CPU-local state and shared cancel-safe driver accounting survive repeated main/handler switches without device-owned callbacks. |

## Active Next Slice

No active implementation slice remains. The v0 model is complete through the v0.8
vector-routed IRQ context slice recorded in `model/docs/pdma_arch_spec_v0.md`.
The scenario suite has 54 tests: 49 software-bearing tests each use exactly one complete
`run_main`, while five named pure device/manual observability tests intentionally use
none, as enumerated in the architecture specification.

## Planned Slices

None. All v0 slices are validated and the architecture spec is drafted.

## Deferred Until v0 Mechanisms Stabilize

- Cycle accuracy, RTL timing, power, or frequency modeling.
- Exact descriptor byte layout and register offsets.
- C ABI and generated headers.
- RISC-V scheduler backend through `terminus`.
- High-dimensional transfer operations, gather/scatter, transpose, and reduce trees.
- Real Linux DMAengine, DPDK DMAdev, or kernel driver integration.
- Concrete PCIe MSI/MSI-X address/data transport, MSI-X table/PBA behavior, and
  APIC/GIC integration.

## Update Rule

When a slice lands, update this file in the same commit or the immediately following docs commit:

- Move the slice to `Progress Ledger` with concrete evidence paths.
- Promote exactly one next slice into `Active Next Slice`, or mark no active slice when v0 is complete.
- Keep `Planned Slices` ordered by dependency, not preference.
- Record new deferred decisions here instead of burying them in commit messages.
