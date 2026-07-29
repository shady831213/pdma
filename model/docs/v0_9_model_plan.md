# PDMA v0.9 ABI Register-Contract Model Plan

## Status

**ACTIVE.** v0.9 is the next implementation slice after the completed v0.8
vector-routed IRQ model. This ledger records approved work in progress; it does not
claim that v0.9 is complete.

## Purpose and Boundaries

v0.9 adds one stable, version-1 PDMA data ABI around the existing deterministic
execution/resource/retirement model. The ABI path is a new queue-I/O frontend, not a
second executor. Contract owns ABI vocabulary, device owns DMA/control/reset runtime
state, host owns bus procedures and software execution, and scenarios own actor wiring
and assertions. The v0.8 CPU, IRQ, cancellation, publication-before-IRQ, and
drain-before-ack boundaries remain mandatory.

The slice includes:

- canonical fixed-width, little-endian descriptor, completion, event, IOVA, queue-ring,
  and typed-control vocabulary;
- host-owned DMA regions/rings and a checked, reset-persistent `DmaSpace`;
- atomic queue activation, producer/consumer controls, PDMA-local IRQ controls, and
  whole-device reset with epoch-bearing stale-capability rejection;
- one shared semantic/DMA execution and retirement core; and
- a generated C data header derived from the canonical Rust metadata plus one migrated
  Message-IRQ end-to-end scenario.

## Locked v1 Protocol

- All wire fields are explicit fixed-width little-endian values: IDs and IOVAs are
  `u64`; version, opcode, record size, status/kind, and flags are `u16`. Rust layout,
  padding, `usize`, and enum representation are never wire data.
- The known feature mask is `0xff`: `IOVA64`, `DESCRIPTOR_V1`, `COMPLETION_V1`,
  `EVENT_V1`, `COPY`, `STRIDED_COPY`, `INDIRECT_BROADCAST`, and `VECTOR_ADD` occupy
  bits 0 through 7. Unknown bits are rejected.
- Descriptor v1 is 64 bytes: header at bytes `0..16`, command ID at `8..16`, then six
  payload words. Opcodes are Copy=1, StridedCopy=2, BroadcastCopy=3, and VectorAdd=4;
  all v1 descriptor flags and reserved words are zero.
- Completion v1 is 32 bytes and event v1 is 64 bytes. Completion success is status 0.
  Event kinds, stage/access values, fault locators, auxiliary details, and first-match
  validation order are the canonical registry defined by the approved v0.9 plan.
- Queue DMA uses monotonic `u64` cursor shadows. Slots are cursor modulo depth; producer
  advancement is submission acceptance, doorbell capture is a separate commit, and
  record bytes commit before producer/trace/IRQ.
- Controls are typed transactions, not numeric MMIO selectors: discovery/limits/status,
  atomic queue activation, cursors, PDMA-local IRQ route/control/observation, and
  whole-device reset. Reset retains host DMA but clears resettable device state, advances
  `DeviceEpoch`, and makes old queue/IRQ capabilities stale before mutation.
- Every DMA extent and arithmetic operation is checked. A malformed accepted descriptor
  consumes once and retires as an event; host encoding failure is pre-acceptance; no
  successful completion follows a descriptor, list, or memory fault.

## Deferrals

v0.9 does not add PCIe, MSI/MSI-X tables/PBA, numeric register/MMIO maps, APIC/GIC,
Terminus, OS drivers, C-callable APIs, firmware/ISA scheduling, timing, RTL, unsafe
layout casts, serialization dependencies, per-queue reset, a second executor, a second
driver, or broad migration of existing semantic scenarios. Concrete transport mapping
and generated headers beyond the ABI data header remain later work.

## Immutable Pre-edit Baseline

The baseline was captured before v0.9 product edits with `cargo test --workspace --
--list`, normalized to sorted unique names. It contains 137 names: 135 Rust tests and
two doctests.

Baseline test-name SHA-256: 15dd6dd3a517b300957e97a0535c9a7aea3ec520a181a347a137578f0a04c72a

The executable scenario inventory is 56 tests, 51 `.run_main(` callsites, five named
zero-entry pure device/manual observability tests, and four explicit
`ManualDriver::setup_on` callsites. The zero-entry exceptions are:

- `reports_step_budget_exhaustion_when_work_remains`;
- `async_runner_no_work_reports_idle_status`;
- `async_manual_run_until_blocked_returns_terminal_status_without_losing_idle_errors`;
- `async_tracing_preserves_manual_run_until_idle_behavior`; and
- `async_trace_log_records_idle_status_steps`.

The four manual setup callsites are the step-budget test, the manual run-until-blocked
test, and baseline/traced drivers in the manual tracing comparison. The complete sorted
name file is evidence-only; this digest is its immutable product-ledger binding.

Plan: .omo/plans/v0-9-abi-register-contract.md
