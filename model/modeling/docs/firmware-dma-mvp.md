# Firmware-managed DMA architecture MVP

Recorded: 2026-09-15.
Status: implemented first milestone, awaiting user architectural review.
This records scope and observable acceptance; the broader goal remains separate.

Parent direction: [firmware-managed DMA](firmware-dma-goal.md).
Follow-on implementation: [independent control channel](control-channel.md).
That follow-on now supports asynchronous multi-channel management and independent
[Function-scoped Host ports](function-isolation.md). The one-channel story below records the original architecture milestone;
current lifecycle and per-channel window semantics are in the linked document.
The current assembly uses explicit acquisition/release instead of opening DMA
as a side effect of firmware readiness.

## Review follow-up: 2026-09-16

The working MVP checkpoint is preserved at `43515cb`. Its standalone
`managed-runtime` crate was a provisional placement, not an architectural requirement.
The follow-up keeps the behavior while changing ownership and reusable mechanics:

- Concrete assembly, transport adapters and their seven boundary tests now live
  under `crates/scenarios/tests/firmware_managed/harness/runtime`. Native module
  paths preserve the tests without source duplication. The extra crate is removed;
  hardware components and firmware remain independent production crates.
- `hardware-interface-irq::interrupt_wire` supplies split emission and
  reception capabilities, reused by both ordinary and managed Host DMA delivery.
  This is not a new firmware communication protocol.
- Firmware now receives management interrupts through its own `InterruptRuntime`.
  Its program registers the ISR before Ready; its CPU remains distinct from Host.
- IO groups memory/register interfaces, supplied byte backing and shared-state storage
  in one std-only crate. DMA consumes its memory capability without a normal Tokio
  dependency. IRQ's Tokio pending backend is optional; SoftwareEnv/main/ISR execution
  belongs to the software crate. None of these packages claims `no_std` support.
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
   the closed access window through management MMIO and publishes its independent boot report.
4. The same Host program waits for firmware Ready within its software deadline,
   allocates control rings and requests a channel. After the grant it configures
   its DMA rings, submits an
   ordinary copy, receives the DMA notification and consumes the response.
5. Release through firmware, waiting for DMA reset completion. Check copied bytes,
   response, notification and closed-window behavior; then end and join all
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
| `firmware` | Boot and service management requests using supplied memory/MMIO capabilities; sequence allocation and real channel reset without accessing DMA internals. |
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

- Host and firmware have independent `SoftwareEnv` instances and capability views.
  Both use `InterruptRuntime` for their independent interrupt inputs.
  Reuse existing implementations and the async executor.
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
- After readiness and explicit acquisition, the same Host program performs a real DMA copy and observes
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

- `contract::hw_components::windows::ChannelWindowRegisters` declares one bank of `ChannelWindowBlock`
  children. Each child has alignment 4 and extent 12, with `WindowControl` at
  byte 0 (open and Function owner) and `WindowStatus` at byte 8. Bank spacing derives from the child layout;
  the schema has no explicit stride. Host and firmware projections filter this
  same bank while retaining its full extent, including hidden leaves.
- `contract::hw_components::windows::WindowControl` is firmware-only, at its platform-selected permission-bank base
  plus the generated per-channel control offset. Writing its `open` bit enables only that
  channel's work access.
- `contract::hw_components::windows::WindowStatus` is Host-readable at its generated status offset
  within each platform-placed permission block. Blocks repeat every twelve bytes. Its `ready`
  value is derived from the same open bit, not a second retained handshake state.
  It remains readable by the bound Function while work access is closed.
- `contract::hw_components::firmware::FirmwareRegisters` is an independent report at a platform-selected base,
  owned by `hw-components::firmware::FirmwareStatus`. Firmware alone writes
  its packed state/error word; Host can read it regardless of DMA permissions.
  Reset initializes Booting/no error. Runtime construction represents subsystem
  reset release; polling the firmware CPU starts the program without image loading
  or validation. DMA and channel reset do not reset this subsystem-level state.
  Live subsystem reboot and watchdog remain outside the current implementation;
  Host/firmware requests now use the separate control rings.
- `hw-components::windows::ChannelWindows` routes transactions to independent `ChannelWindow`
  objects. Each window owns owner/open registers, requester authorization, readiness,
  complete-transaction bounds and protection of its management registers. The bank
  owns instance layout, public discovery, global/admin isolation and window lookup;
  it owns no DMA register copy or software policy.
- `firmware::main` closes the work window on its own software CPU and publishes
  Ready on success or Failed/WindowSetup on rejection. After Ready it serves the
  independent control rings, allocating a free channel on request and closing/resetting
  it before confirming release. Ordinary management requests are served serially;
  independently driven peer DMA is unaffected. A boot failure remains idle; report or service
  transport errors reach the supervisor. Runtime never fabricates a firmware report.
  Shared in-memory queue state is bootstrapped through always-on registers.
  Bidirectional doorbells generate management interrupts on publication and space
  return. Each program's registered ISR wakes its processing task. Host keeps DMA
  delivery on a separate vector. See `control-channel.md`.
- `host::firmware::wait_ready` runs on the Host CPU with a caller-selected deadline.
  Timeout/cancellation does not reset hardware or stop firmware. It does not grant
  DMA permission or load firmware; Host uses `Management` to request access.
- The harness's `runtime::Managed::with_channels` selects the count and returns an
  undriven system and Host capabilities; `new` selects one channel.
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
cargo test -p hardware-interface-irq --test interrupt_wire
cargo test -p hardware-interface-software --test interrupt_wire_delivery
cargo test -p pdma-model-scenarios --test firmware_managed
cargo check -p pdma-model-dma --lib
cargo tree -p pdma-model-dma --edges normal
cargo test -p hardware-interface-software --no-default-features
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
