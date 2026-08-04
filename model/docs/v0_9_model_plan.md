# PDMA v0.9 ABI Register-Contract Model Plan

## Status

**PARTIAL.** The historical v0.9 descriptor-layout ABI register-contract slice is
complete after the v0.8 vector-routed IRQ model, but the overall v0.9
memory-and-bus-exact objective remains partial. The approved active correction slice is
an exact 4 KiB numeric MMIO model; no MMIO implementation exists yet.

## Active Correction Slice

Implement the approved exact 4 KiB numeric MMIO model while preserving the completed
descriptor, completion, and event ABI record below and every v0.8 IRQ/CPU-local
boundary. Numeric MMIO is active work, not a completed v0.9 claim.

## Historical Completed Descriptor-Layout ABI Baseline

The following completed descriptor-layout ABI baseline is retained verbatim for the
active correction slice.

### Purpose and Boundaries

v0.9 establishes one stable, version-1 PDMA data ABI around the existing deterministic
execution/resource/retirement model. The ABI path is the canonical path for new
end-to-end scenarios, not a second executor; existing semantic scenarios remain valid
coverage of that same behavior. Contract owns ABI vocabulary, device owns DMA/control/
reset runtime state, host owns bus procedures and software execution, and scenarios own
actor wiring and assertions. The v0.8 CPU, IRQ, cancellation, publication-before-IRQ,
and drain-before-ack boundaries remain mandatory.

The slice includes:

- canonical fixed-width, little-endian descriptor, completion, event, IOVA, queue-ring,
  and typed-control vocabulary;
- host-owned DMA regions/rings and a checked, reset-persistent `DmaSpace`;
- atomic queue activation, producer/consumer controls, PDMA-local IRQ controls, and
  whole-device reset with epoch-bearing stale-capability rejection;
- one shared semantic/DMA execution and retirement core; and
- a generated C data header derived from the canonical Rust metadata plus one migrated
  Message-IRQ end-to-end scenario.

### Locked v1 Protocol

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
  whole-device reset. A successful reset checked-advances `DeviceEpoch`, preserves the
  committed `DmaSpace` mappings, bytes, and allocator generation plus trace history,
  resource configuration, and lifecycle accounting, and clears queues,
  completion/event records, runnable work, pending retirement, bus-control queue state,
  and PDMA-local IRQ state. Queue/source/notification capabilities from the prior epoch
  are rejected as stale before mutation; rejected stale reset/IRQ operations and epoch
  overflow mutate nothing. Fresh bootstrap may reuse queue/source numeric IDs only under
  the new epoch.
- Every DMA extent and arithmetic operation is checked. A malformed accepted descriptor
  consumes once and retires as an event; host encoding failure is pre-acceptance; no
  successful completion follows a descriptor, list, or memory fault.

### Historical Deferrals

v0.9 does not add Terminus, PCIe/MSI-X numeric address/data maps or table/PBA behavior,
numeric register/MMIO maps, APIC/GIC, OS drivers, a callable C API, firmware/ISA
scheduling, timing, RTL, unsafe layout casts, serialization dependencies, SR-IOV, FLR,
per-queue reset, a second executor, a second driver, a second end-to-end scenario, or
broad migration of existing semantic scenarios. The generated C data header is not a
callable API; concrete transport mapping and every header beyond that data header remain
later work.

### Immutable Pre-edit Baseline

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

### Completion Evidence and Header Policy

The final scenario inventory remains exactly 56 tests, 51 `.run_main(` callsites, the
five zero-entry exceptions above, and four `ManualDriver::setup_on`-family callsites.
The historical ABI inventory retains its 137-name pre-ABI baseline. The later exact-MMIO
Todo 1 baseline contains 189 names with SHA-256
`bb903a3f87d76ac34eafa0bdff605d29c02362d1a7ac8da7c897068d598d360b`; exact-MMIO
coverage removes none of those 189 names.
The canonical Message-IRQ copy scenario now uses `BusDevicePort<ScenarioBus>` backed by
`MmioDeviceBus<RecordingMmioTransport>`. Its ordered batches carry ACTIVATE and producer
publication, while queue/IRQ commands and final raw status observation traverse direct
MMIO. It preserves the nine-stage lifecycle, vector 64, one delivery and device step,
all required cursor values, `MainAfterHandler`, and destination bytes `[1, 2, 3, 4]`.

`include/pdma_model_abi.h` is a checked-in generated C11 *data* header rendered from
canonical Rust ABI metadata. Do not edit it by hand. Whenever that metadata changes,
regenerate it with `cargo run -p pdma-model-contract --bin pdma-abi-header -- --write
include/pdma_model_abi.h`, review the generated diff, and require `cargo run -p
pdma-model-contract --bin pdma-abi-header -- --check include/pdma_model_abi.h` in every
relevant gate. The check is byte-for-byte and must fail for a stale copy.

Final Todo 19 evidence is recorded under
`.omo/evidence/v0-9-abi-register-contract/`: `task-19-red.log` pins the active
pre-edit ledger, digest, inventory, and fresh header; `task-19-full-gates.log` records
the workspace and manual lifecycle gates; `task-19-full-gates-error.log` records the
focused/adversarial checks; and `task-19-implementation-report.md` binds them to the
documentation commit.
