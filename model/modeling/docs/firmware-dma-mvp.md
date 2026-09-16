# Firmware-managed DMA architecture MVP

Recorded: 2026-09-15.
Status: implemented first milestone, awaiting user architectural review.
This records scope and observable acceptance; the broader goal remains separate.

Parent direction: [firmware-managed DMA](firmware-dma-goal.md).

## Review follow-up: 2026-09-16

The working MVP checkpoint is preserved at `43515cb`. Its standalone
`managed-runtime` crate was a provisional placement, not an architectural requirement.
The follow-up keeps the behavior while changing ownership and reusable mechanics:

- Concrete assembly, transport adapters and their seven boundary tests now live
  under `crates/scenarios/tests/firmware_managed/harness/runtime`. Native module
  paths preserve the tests without source duplication. The extra crate is removed;
  hardware components and firmware remain independent production crates.
- `hardware-interface-runtime::pending_interrupt` supplies split emission and
  reception capabilities, reused by both ordinary and managed Host DMA delivery.
  This is not a new firmware communication protocol.
- Firmware with no interrupt inputs runs directly through `SoftwareCpu::run_main`,
  without an idle source, handler or dispatcher. Its CPU remains distinct from Host.
- The framework's default-on `async-runtime` feature gates CPU/interrupt execution
  and Tokio. DMA disables defaults and builds independently without a normal Tokio
  dependency. Synchronous interfaces and backend implementations remain in the same
  crate: this does not claim a strictly API-only or `no_std` package.
- A generic async shared-context wrapper was evaluated and not added. Concrete
  state projection and IRQ extraction still differ; retaining the small explicit
  mutex/wakeup adapters avoids extra hook/trait machinery. Software accesses still
  wake the single hardware worker, including reads and error results; steps do not
  wake themselves into a busy loop. No new supervisor framework is introduced.

## Purpose

Prove that DMA hardware, management hardware components, firmware, concrete
runtime assembly and scenario harness can stay within their respective ownership
boundaries. Run one real copy through that structure before adding SR-IOV, PASID
or Function reset. A successful build alone is not acceptance.

## Minimal running story

1. Assemble one DMA channel and a work MMIO window that starts closed. Give Host
   only the work view and firmware the privileged management view.
2. Host attempts a channel access while the window is closed and observes an
   access error, with no forwarded channel side effect.
3. Run the firmware async program on its own software CPU. Firmware initializes
   the access window through management MMIO and publishes its independent boot report.
4. The same Host program waits for firmware Ready within its software deadline,
   verifies the separate window permission, configures its rings, submits an
   ordinary copy, receives the DMA notification and consumes the response.
5. Check copied bytes, response and notification behavior; then end and join all
   actors through the concrete runtime/harness lifecycle boundary.

Host allocates and initializes application buffers through the supplied allocator.
Neither firmware nor the harness preallocates Host buffers. The harness may control
actor startup order to observe the closed window, but may not perform the firmware's
management writes or manufacture readiness. Define how Host observes readiness
without requiring access to the closed channel window.

## Placement and responsibility

| Part | MVP responsibility |
| --- | --- |
| `contract` | Minimal shared management/work register definitions and observable access outcomes. No context selector or PCIe ABI additions yet. |
| `dma` | Core renamed from `device` without changing ordinary behavior; retains its channel state and shared copy pipeline. |
| `hw-components` | Implement one real access-window hardware object with firmware-owned configuration access; reject or forward Host MMIO to the original channel state. |
| `firmware` | Implement the async initialization program using supplied management access and software execution capabilities, not DMA internals. |
| Host software | Use only its work/readiness/memory/interrupt capabilities; reuse the existing channel driver where applicable. |
| Concrete runtime assembly | Connect hardware and software ports, drive hardware and firmware, and define startup/error/shutdown propagation in the scenario harness using existing runtime components. |
| `scenarios` | Keep an isolated `firmware_managed` test suite with cases and concrete assembly/harness support, not production hardware behavior, firmware policy or generic runtime mechanisms. |

The concrete assembly is `crates/scenarios/tests/firmware_managed/harness/runtime.rs`
and its native submodules; it reuses the existing software CPU, interrupt runtime
and hardware executor. It is not a new runtime framework
or one crate per small hardware object. Ordinary single/multi-channel scenarios
continue to use the direct DMA assembly; path/package updates for the core rename
must not make them depend on firmware or `hw-components`.

## Runtime and access boundaries

- Host and firmware have independent `SoftwareCpu` instances and capability views.
  Host uses `InterruptRuntime`; firmware currently has no interrupt inputs and
  runs directly on its CPU. Reuse existing implementations and the async executor.
- Host and firmware receive different port sets. Host cannot access window-control
  registers, and closing the work window does not disable firmware's management
  path. Do not pass either program a mutable DMA object or an unrestricted system
  handle and rely on convention to restrict its use.
- Hardware objects own their register state and enforce access. Firmware owns the
  initialization policy. Runtime owns transport, driving and wakeups, not either
  behavior. Reuse schema register windows/dispatch and existing stage combinators
  where applicable rather than introducing duplicate mechanisms.
- The memory backend remains synchronous for this MVP. Keep storage selection
  outside DMA and share actual channel/register state, not copied views.
- Construction must not create unowned background actors. A firmware initialization
  error and a hardware/dispatcher failure must be observable by the supervisor;
  successful initialization must have a defined continuing or terminal firmware
  state. Waiting firmware must not prevent Host or hardware progress.
- Define normal test completion and early failure cleanup. Stop/join all owned
  actors without claiming that cancellation of simulation proves hardware reset,
  transaction drain or permission to reuse in-flight memory.

## Acceptance

The tests below exercise these behavioral checks. Test names are coverage pointers,
not a substitute for running the verification commands on the current checkout.

- Before firmware opens the window, Host channel accesses fail and cannot alter
  channel state; attempts to program management registers through the Host view
  remain rejected after the work window opens.
- Firmware management access works while Host access is closed. Readiness becomes
  visible only after firmware performs the required configuration writes.
- Firmware reports Booting, Ready or Failed independently of the window permission.
  Host cannot forge this report. A reportable setup failure stays observable without
  ending simulation; a stuck firmware is detected by Host timeout, not a fabricated
  hardware error. CPU or status-transport failure remains an execution error.
- After readiness, the same Host program performs a real DMA copy and observes
  exact destination bytes, a successful matching response and the expected Host
  notification using the existing interrupt runtime.
- CPU ownership and interrupt dispatch remain separate between software domains;
  no Host handler executes as firmware or vice versa. Waiting firmware does not
  stall the copy/Host path.
- Initialization failure and early shutdown produce an explicit result and leave
  no orphan hardware, firmware or dispatcher tasks.
- Existing single/multi-channel copy, SG, fault, reset and interrupt scenarios
  remain valid without enabling the managed assembly.
- Source dependencies reflect the table above: no production firmware/hardware
  implementation in scenarios, no firmware policy in runtime, no shadow channel
  registers and no direct software mutation of hardware internals.

## Not in this MVP

No PCIe integration, PF/VF allocation, Function reset/FLR, context table, PASID or
command-format extension, context rebinding, ATS/PRI, delayed external transaction
model, multi-version mapping, task-context ledger or new scheduling policy. Do not
add an ISA simulator or a second executor/interrupt framework. Broader lifecycle
requirements remain in the parent goal and must not be claimed as validated by
this access-window demonstration.

## Implemented interfaces and verification

- `contract::WindowControl` is a firmware-only generated register map at
  `WINDOW_CONTROL_BASE` (`0xffff_ff00`). Writing its `open` bit enables work access.
- `contract::WindowStatus` is a Host-readable generated map at `WINDOW_STATUS_BASE`
  (`0xffff_ff08`). Its `ready` value is derived from the same open bit, not a
  second retained handshake state. It remains readable while work access is closed.
- `contract::FirmwareRegisters` is an independent report at `FIRMWARE_STATUS_BASE`
  (`0xffff_ff10`), owned by `hw-components::FirmwareStatus`. Firmware alone writes
  its packed state/error word; Host can read it regardless of DMA permissions.
  Reset initializes Booting/no error. Runtime construction represents subsystem
  reset release; polling the firmware CPU starts the program without image loading
  or validation. DMA and channel reset do not reset this subsystem-level state.
  Live subsystem reboot, watchdog and a Host/firmware request protocol are not
  implemented by this boot-status slice.
- `hw-components::AccessWindow` protects the complete downstream DMA register
  extent, including rejection of spanning/overflowing accesses and Host attempts
  to access management registers. It owns no DMA register copy or software policy.
- `firmware::main` performs the window setup on its own software CPU and publishes
  Ready on success or Failed/WindowSetup on rejection. Either report remains idle
  until simulation cancellation. If reporting itself fails, the transport error
  reaches the supervisor instead. No runtime code fabricates a firmware report.
  No management interrupts are generated; firmware has no interrupt runtime.
- `host::firmware::wait_ready` runs on the Host CPU with a caller-selected deadline.
  Timeout/cancellation does not reset hardware or stop firmware. It does not grant
  DMA permission or load firmware; Host checks the window separately before use.
- The harness's `runtime::Managed::new` returns an undriven system and Host capabilities.
  `run` scopes hardware and firmware futures. `run_until` additionally supervises
  a supplied Host execution; actor errors take priority over ready Host completion.
  No actor is spawned during construction, and neither run method detaches tasks.
- The scenario harness yields once before polling firmware so the same Host program
  can first observe the closed window. Host has no startup sender or harness handshake;
  its only firmware readiness input is the hardware status register.
  Concrete wiring belongs to the harness; firmware policy and hardware behavior
  remain in their production crates.

From `model/modeling`:

```sh
cargo test -p pdma-model-hw-components
cargo test -p pdma-model-firmware
cargo test -p hardware-interface-runtime --test pending_interrupt --test pending_interrupt_delivery
cargo test -p pdma-model-scenarios --test firmware_managed
cargo check -p pdma-model-dma --lib
cargo tree -p pdma-model-dma --edges normal
cargo test -p hardware-interface-runtime --no-default-features
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

`hw-components/tests` covers gate permissions and transactional boundaries.
`firmware/tests/boot.rs` covers software CPU and management transport failures.
The scenario's `harness/runtime/run/tests.rs` covers actual assembly CPU separation,
scoped cancellation, real firmware failure through the supervisor seam and error
priority; `harness/runtime/tests.rs` covers work-view behavior. The scenario suite
checks an IRQ-driven copy and early Host failure with all runtime owners released.
Check the DMA dependency tree on its own: workspace feature unification can enable
async support for other consumers and would conceal the standalone boundary.
