# PDMA v0 Model Progress Plan

## Purpose

This file tracks executable v0 model progress. White-board documents explain architecture direction; this plan records what the Rust model has already proven, what is active next, and what must stay deferred until the current mechanisms are validated.

## Current Ground Rules

- Keep v0 deterministic and step-based with `step()` and `run_until_idle()` scenario tests.
- Keep boundaries async-friendly, but do not require an async runtime.
- Prefer behavioral scheduler lowering over cycle-accurate timing.
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
| Device step loop | Done | `device.rs`, `linear_copy.rs` | `step()` executes at most one command; `run_until_idle()` reports step budget exhaustion. |
| Linear copy lowering | Done | `scheduler.rs`, `engines.rs`, `linear_copy.rs` | One external copy lowers to one internal chunk and one completion. |
| 2D strided copy lowering | Done | `strided_copy.rs` | One external strided command lowers to row chunks and one completion; checked memory errors do not publish success. |
| Event ring and recoverable error reporting | Done | `event.rs`, `error_event.rs` | Memory faults publish software-visible events, skip success completions, and keep later queued commands runnable. |
| Queue directory and explicit queue configuration objects | Done | `queue.rs`, `queue_config.rs` | Queue creation stores directory entries, driver owns config handles, and queue-local event polling preserves ownership. |
| Multi-queue arbitration with deterministic policy | Done | `arbitration.rs`, `multi_queue.rs` | Default round-robin policy selects oldest runnable queues and prevents equal-priority queue starvation. |

## Active Next Slice

### Broadcast Copy Lowering

Goal: validate one external command lowering into multiple destination writes while preserving the existing completion and error-reporting contract.

Expected behavior:

- One broadcast copy command reads one source range and writes the same payload to multiple destination ranges.
- The scheduler lowers one external broadcast command into internal copy work for every destination.
- The command publishes one success completion only after all destination writes complete.
- A failed destination write reports an event and does not publish success.
- Existing linear, strided, event, queue-configuration, and arbitration behavior remains unchanged.

Likely files:

- `model/modeling/src/desc.rs`
- `model/modeling/src/scheduler.rs`
- `model/modeling/src/engines.rs`
- `model/modeling/src/device.rs`
- `model/modeling/tests/broadcast_copy.rs`

Success checks:

- Scenario test proves one command copies the same source bytes to two or more destinations and reports one completion.
- Scenario test proves an out-of-bounds destination reports an event and prevents success completion.
- Existing linear, strided, error-event, queue-configuration, and arbitration tests continue to pass.

## Planned Slices

1. Broadcast copy lowering to validate one-command fanout.
2. Vector-add operation to validate multi-source reads and compute/write synchronization.
3. Resource credit model for internal descriptors, read slots, write slots, compute slots, and completion/event pressure.
4. Backpressure/stall annotation layer with scenario-level summaries.
5. Trace sink with Chrome/Perfetto-compatible JSON events plus a structured event log.
6. Draft `model/docs/pdma_arch_spec_v0.md` after the model validates queue, scheduler, completion, event, and backpressure behavior.

## Deferred Until v0 Mechanisms Stabilize

- Cycle accuracy, RTL timing, power, or frequency modeling.
- Exact descriptor byte layout and register offsets.
- C ABI and generated headers.
- RISC-V scheduler backend through `terminus`.
- High-dimensional transfer operations, gather/scatter, transpose, and reduce trees.
- Real Linux DMAengine, DPDK DMAdev, or kernel driver integration.

## Update Rule

When a slice lands, update this file in the same commit or the immediately following docs commit:

- Move the slice to `Progress Ledger` with concrete evidence paths.
- Promote exactly one next slice into `Active Next Slice`.
- Keep `Planned Slices` ordered by dependency, not preference.
- Record new deferred decisions here instead of burying them in commit messages.
