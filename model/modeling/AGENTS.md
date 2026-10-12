# PDMA MODELING WORKSPACE

## Current state

This workspace is the starting point for a clean PDMA rebuild.
The old contract, device, host, and retired sync implementation were deliberately
removed from master after backing up the complete source snapshot:
`backup/pdma-before-rebuild-20260909` at
`51707fe1d38e261a188e1d0ac14243c0caf58127`.

The replacement now implements instance-configured channels with command and response rings,
linear/scatter-gather DMA copy, error/reset boundaries, and edge/IOC IRQ delivery. See README.md
and the `crates/scenarios/tests/{single_channel,multi_channel}/main.rs` system tests. Old interfaces, numeric register
layouts, and ABI compatibility are not requirements for this model.

## Active building blocks

| Crate | Responsibility |
| --- | --- |
| hardware-interface-pipeline | Typed stages, generic gathering/distribution, composition, and driving policies. |
| hardware-interface-runtime | Memory backends, register access/observation, CPU-local state, interrupt dispatch, and trace storage. |
| hardware-interface-schema | Runtime-neutral record/register schemas, metadata, access, and exporters. |
| hardware-interface-schema-macros | Schema code generation and compile-time validation. |

Keep these crates and their tests intact. Reuse their existing capabilities in the
replacement model; do not introduce parallel composition or dispatch mechanisms.

## Historical references

The obsolete scenarios sources are preserved on the backup branch only. The new
`crates/scenarios` owns system tests against the replacement model's interfaces;
individual crates retain their focused boundary tests. Scenario assembly, actor
shutdown/join stay in the test harness; case modules own inputs and acceptance assertions,
with one complete software program inside one `InterruptRuntime::run_main` per case.
Keep cases, hardware assembly, software program and harness in distinct native modules.
Software owns ring/payload buffer allocation,
payload initialization,
queue configuration and handler construction/registration; the harness must not
perform those operations for it. The harness selects memory backing and creates the
allocator service; it does not preallocate application buffers. Software receives
allocation, bus/memory capabilities and input
data, not a device core or executor. Hardware assembly receives memory access, not
software handlers or CPU state. Software calls the runtime-supplied ISR registration
capability; CPU validation, handler tables and dispatcher lifecycle belong to the
runtime. Do not expose registry freezing, startup handoff channels or dispatcher tasks
to the software program. `host::runtime` owns PDMA transport, separate from the host
channel driver; generic CPU/interrupt mechanics stay in `hardware-interface-runtime`.
`CpuOwner` is runtime-private. Software entrypoints and ordinary drivers must not
capture CPU ownership tokens or bind their state to the CPU that created them.
Management clients, rings and DMA drivers follow normal Rust ownership and borrowing;
their retained operations may continue on another CPU when the supplied capabilities
permit it. Explicit CPU-local state and CPU-specific register capabilities keep their
runtime checks. Main/ISR scheduling and registration remain runtime responsibilities.
Host test programs observe software-visible memory
and registers, not synthetic harness notifications of individual DMA writes.

`include/pdma_common.h`, `include/pdma_host.h` and `include/pdma_firmware.h` are generated from the current contract schemas by
`pdma-abi-header` with the optional `export-c-header` feature. Regenerate and check
them using the commands in `include/README.md`; never edit them by hand. The old ABI
header is preserved on the backup branch, not in the active include directory.

The obsolete architecture specification and version plans formerly under
`model/docs` are retained in Git history only. They must not force old software
interface shapes into the rebuild. The workspace README and `docs/reset.md` describe the
active model; `docs/firmware-dma-goal.md` records the agreed next direction, not
implemented functionality or an instruction to start it automatically.
`docs/context-contract.md` records Default/PASID context semantics and numbered
software/hardware obligations for future RTL specification and tests. The optional
context table owns bindings in hw-components; DMA owns producer-frontier snapshots
and accepted-command retirement. Preserve these boundaries and the software
obligations explicitly; do not turn them into hidden harness policy.
The approved first milestone is `docs/firmware-dma-mvp.md`: validate the `dma`,
`hw-components`, firmware and concrete runtime boundaries through a real copy
behind a firmware-configured access window. Production hardware and firmware
belong outside scenarios. Concrete managed assembly and transport adapters belong
in the scenario harness, superseding the earlier wiring placement requirement.
Keep the direct-DMA assembly available for ordinary tests.
The DMA core lives in `crates/dma`. The `firmware_managed/harness/runtime` modules
assemble the MVP over `hw-components::windows::ChannelWindows` and `firmware::main`.
`hw-components::{windows,context,control,firmware}` groups each hardware object with
its memory adapters and errors. Cross-object bus errors belong to the managed runtime;
context construction uses its own capacity/layout/allocation errors, not window errors.
Each `ChannelWindow` owns its registers, requester authorization, open/readiness state
and transaction bounds. The bank owns only instance layout, discovery/global routing,
transaction-envelope validation and window lookup. Function-bound ingress delegates
to the selected window; do not duplicate owner/open/status policy in the bank.
Host and firmware use separate software CPUs and interrupt runtimes. Each program
registers its management ISR; Host has a separate DMA vector. Firmware boot status is separate
from window permission: Booting/Ready/Failed and a boot error are firmware-published
in an always-on register. Window readiness still derives from its open bit.
Host waits for firmware with a software deadline before checking DMA permission.
Independent control command/response rings now carry channel acquisition/release;
Ready does not grant access. Geometry, cursors and entries now live in memory;
bootstrap/enable and bidirectional doorbells are hardware registers. Firmware caches
validated geometry once; software owns cursor updates. `docs/control-channel.md`
defines their ownership and cancellation contract. The independent PF event ring,
event doorbell and firmware-owned event error follow docs/fatal-events.md. The system
specification entry point is docs/spec.md; firmware is part of that specification.
DMA fatal interrupts reach firmware only; firmware closes windows and reports PF
events even with full response storage. ResetDma preserves allocations, bindings and
management state, leaving data windows closed. ResetEvents repairs only failed event
delivery. Event full is an error, never backpressure or an implicit retry queue. Hardware owns registers/permissions, firmware consumes and
executes management requests, and Host owns ring memory. Runtime carries no resource
policy. A single PF management client issues bounded asynchronous requests naming target
Functions over one control ring pair. VF software only uses granted data channels.
Hardware checks the platform-provided PF identity on control-register access. Firmware
validates request targets against its configured Function namespace, retains per-channel
owner allocation state and serves requests serially. A full PF response ring stops admission.
ResetChannel closes its data window, awaits hardware reset, then reopens unconfigured data
access without releasing ownership. ReleaseChannel awaits reset before freeing ownership.
Later management requests wait; independently driven peer DMA continues. Resource lifecycle
methods stay separate from control-ring receive/respond. The agreed future VF FLR
sequence requires PF to collect successful releases for all target channels before
FLR and prevent reacquisition until actual FLR completion. Do not add firmware FLR
ingress, repeated cleanup or a resource-precondition checker for that contract.
The application endpoint may ACK immediately; PCIe drain and controller handshake
belong to integration. Direct unprepared FLR is unsupported, PF FLR is a separate
scope, and neither is implemented. See the VF pre-FLR section in `docs/function-isolation.md`.
Discovery is always readable, global/channel administration is firmware-only in managed mode,
and grants expose only complete schema-defined channel data blocks. Platform assembly supplies
register bases and validates disjoint apertures. Response backpressure stops request admission. Do not turn simulation
cancellation into reset/drain or reclaim ring memory.
No shadow channel registers or firmware writes in the harness are allowed.
The Function-isolation follow-on is documented in `docs/function-isolation.md`.
DMA remains Function-unaware; peripherals enforce MMIO bindings and identify interrupt
destinations before reassignment. Pending messages stay with their original destination.
`MemoryAccess<S = ()>` carries explicit sideband on tagged accesses; ordinary
read/write methods are available only for unit sideband. DMA emits DmaAccess channel
and context selectors. hw-components resolves the same ChannelWindows owner registers
used for MMIO/IRQ routing. Without a context table it forwards FunctionId; with a
table it validates the context owner and forwards a MemoryIdentity with extensible
Default/PASID attributes. Runtime selects injected memory capabilities, not bindings.
The routing adapters allow platform sideband construction from the authorized
source and resolved identity. Opaque fault tags belong to the platform, not DMA or
MemoryIdentity; an MMU echoes them without understanding PDMA channel semantics.
Fault reporting must preserve source correlation when channels share Function/PASID
and VA. Association lifetime includes pending reports and software handling; never
reuse a tag for another source while either can still refer to it. Stable source
tags do not identify fault episodes or replace grant/reset/release exclusion.
Platform assembly selects backing and provides each software program its own allocator.
`MemoryAccess::Error` is backend-owned. Generic runtime adapters and typed access
helpers preserve it; do not extend the runtime's built-in storage error enum for
platform-specific failures. DMA classifies input errors through `Into<MemoryFailure>`
at payload access, while metadata failures retain their scoped error/fatal semantics.
Hardware routing distinguishes its own rejections from downstream errors, and
software error types retain backend details without depending on DMA classification.
Firmware's control memory uses the separate PF-programmed, enable-frozen configuration,
without reserving any data context slot.
Context maintenance takes one atomic context-ID plus Drain/Abort register write.
Hardware retains validity; firmware performs Destroy by awaiting Drain success and
then clearing valid under the documented submission exclusion. The failed-channel
bitmap is the final context-map bank and stays frozen until the next accepted request.
Bindings remain stable until reset completes; no live rebind or shadow owner table.
Independent spaces may use identical addresses. This adds no PASID/IOMMU translation,
Function reset/FLR or external transaction semantics.

The generic runtime's default `async-runtime` feature gates CPU/interrupt execution
and its Tokio dependency. DMA selects `default-features = false`; synchronous
memory/backend APIs stay available and unchanged. hw-components also disables default
features when using the synchronous MemoryAccess traits. Verify both normal dependency
trees independently because workspace feature unification can enable async support.
Both concrete transports use `pending_interrupt`: hardware-side adapters hold the
emitter, software dispatch holds the source. Keep causes, masking, routing policy
and hardware-step wakeup decisions out of that generic component.

## Implementation constraints

Contract public paths follow interface ownership: `contract::dma` for DMA registers,
descriptors and outgoing selectors; `contract::hw_components::{context,windows,control,firmware}`
for peripheral hardware interfaces; `contract::control` for Host-to-firmware software
messages and memory-resident control queues; `contract::memory` for resolved Function
and address-space identities. Shared ring protocol, register primitives and export
projections remain in `ring`, `register` and `view`. Do not restore flattened root
type exports or place hardware bootstrap/doorbell registers among software messages.
Firmware can consume DMA and peripheral contracts without owning those interfaces.

- The replacement models statically configured channels, command/response rings, linear/SG DMA copy,
  independently masked non-empty/IOC notifications, and channel/device error/reset boundaries.
  Do not implement further slices without a user request.
- The constructor fixes the channel count for each instance; software only reads it.
  Contract layout/export takes that count explicitly, with generated child register blocks.
  Register banks repeat complete child blocks; their spacing derives from child extent
  and alignment, with no placement stride override. Reserved gaps belong to their
  enclosing block and cannot host sibling placements or interleaved banks. Group
  related registers in one child block before repeating it.
  One shared execution pipeline processes commands selected by round-robin arbitration,
  with bounded per-channel retained operations for payload-fault suspension.
  CommandArbiter uses Arbitrate (the singleton specialization of FanIn) over
  channel-bound execution ports, with the RoundRobin policy owned by dma. A grant
  either fetches a new command or restores that channel's retained operation.
  Ready checks do not consume commands; eligibility belongs to DMA, not the
  generic combinators. Fetch remains the new-command acceptance stage.
  Route tagged responses through Dispatch (the singleton FanOut specialization) to
  channel-bound publication ports.
  Complete hardware assembly uses Dma::command_loop(); scenarios drive one retained
  operation and abort/join it after software finishes. Do not recreate command-local
  completion/reset/fault recovery in a scenario-side loop. Single-command boundary
  tests may assemble existing stages in test fixtures, not a production factory.
  Skip full-response channels before accepting commands. Preserve per-channel parked
  command/cursor/scratch state without adding engines or physical-channel oversubscription.
  Bind accepted work to channel identity and generation so reset invalidates only its work
  before the next side effect, including after subsequent reconfiguration.
  Follow the request/cleanup/completion contract in `docs/reset.md`: reset write one
  requests cleanup; read one means busy; only hardware clears zero after affected accepted
  work retires. Zero writes do nothing and repeated requests coalesce. Reject configuration
  writes in a busy scope with `Error::ResetInProgress`; retain read access.
  Use `AfterEach<CommandArbiter<Pipeline>, ResetControl>` for the command loop. Track accepted
  channel/generation ownership, including parked references, until terminal release;
  generation invalidation alone
  is not completion. `RunCommand` owns executing-state release at complete pipeline
  termination. A payload fault retains the leaf's Transfer and returns Pending.
  CommandArbiter retains at most one exact composed Operation per channel, preserving
  scratch and custom child-stage state; Channel.parked retains accepted references.
  The granted execution port restores that Operation on resume without refetching,
  restarting CheckRanges or repeating completed transforms. Arbitration retains
  a healthy command's grant through termination and yields it on payload fault.
  CommandArbiter cleans reset/Abort targets before stepping shared active work,
  including a disabled peer. Parking is not whole-command retirement.
  Response publication and `Resettable` binding must not retire the whole command.
  Custom sustained command assemblies pass their downstream transform to
  `Dma::arbitrate`, preserving RunCommand terminal ownership, and drive `ResetControl`.
  `RunCommand<S>` constrains the child stage's associated types, not its concrete composition;
  reuse `Operation` to discard terminal state before releasing command ownership.
  Guard command, SG-list, payload and response accesses before old work can interact again.
  Memory calls are synchronous within a step: do not invent bus transactions, outstanding
  queues, cycle timing or timeout delays. Host reset waits for hardware completion; caller
  cancellation does not release affected memory or cancel hardware cleanup.
- Source and destination independently select linear spans or SG lists. Software owns
  list/buffer allocation and keeps descriptors stable until completion. Hardware checks
  both selected ranges before payload writes, reads lists from explicit start positions
  incrementally and retains only current cursors. One command copy_length selects
  independent Linear byte offsets or SG entry/local offsets; backing totals need not match;
  do not cache whole lists, reject overlap or invent segment-count/total-length limits.
  Fixed copy scratch size is an implementation detail, not a command-size limit.
- Reuse FanIn/Arbitrate/FanOut/Dispatch/Then/Repeat/AfterEach for execution composition. Runtime supplies memory and
  driving only; it must not gain PDMA traversal, validation or copy policy.
- Queue state and its registers belong to the hardware object; actions compose through
  existing stages. Runtime assembly selects memory backend and execution policy.
- The shared `Ring`, `RingPointers` and `RingError` definitions live in `contract`;
  PDMA entry geometry and pointer rules are protocol, even with an `Entry` parameter.
  `software::ring` owns `RingConfig`, `SoftRing` and their cursor I/O adapters;
  host retains its `RingConfig` reexport. Host and firmware share this PDMA software
  crate, whose normal dependencies are contract, generic runtime and schema, never host, firmware
  or DMA. Its `dma` module also owns the shared queue driver and register-only
  `ChannelControl`; firmware uses the latter for reset without owning DMA rings.
  Generic schema/runtime expose no ring protocol or ring types and have no
  normal dependency on PDMA crates. Transport, CPU and notification mechanics stay
  in the generic runtime. `SoftRing` supplies exclusively borrowed software operations with
  borrowed MMIO or memory cursors. Host/device
  keep side-specific memory/MMIO access and publish pointers only after successful
  entry access. Do not introduce a shadow cursor store or duplicate ring arithmetic.
- Keep the model deterministic and step-based. No cycle timing or extra executors.
- Device notification masks never replay old events when cleared. Platform-pending
  notifications survive device masking/reset until claimed. Stopping response errors halt
  the channel after publication; reset never rolls back external memory writes.
  Payload PageFault parks the original command without a response or DMA IRQ/event.
  Retain both cursors, committed prefix and successful read scratch across a failed
  write. Failed PageFault writes commit no external bytes from that call. Same-channel
  successors wait; peers remain eligible. Platform fault reports carry repair identity,
  address and access independently. Software observes 32-bit FAULT_STATUS.faulted
  and writes 32-bit RESUME.request=1 through owned channel MMIO after repair.
  Resume clears faulted without enabling the channel or clearing errors; zero and
  requests while not faulted do not prearm future recovery. Software must serialize
  repair/resume with reset/release/reallocation and retire old handlers; no hardware
  protection against late or duplicate requests for another fault episode is promised.
  Reset cancels parked work without resume; Drain returns ChannelStopped and
  bitmap; Abort discards matching parked work without response, including work that
  parks after Abort acceptance. Metadata faults retain their stopping/fatal behavior.
  Response is 24 bytes with statuses 0..5; PageFault status 6 and fault info are retired.
  See docs/page-fault-recovery.md for normative hardware, software and RTL obligations.
- Software handlers remain outside the hardware. Preserve CPU-local and handler
  execution guarantees supplied by the runtime.
- Runtime memory installs regions atomically with `add_regions(&regions)`. Keep overlap,
  capacity, allocation-failure atomicity and concurrent-writer serialization guarantees.
  Do not restore public prepare/install tokens, transaction drafts or memory revisions.
  Allocation and backing ownership remain outside hardware; the allocator advances its
  cursor only after successful installation.
- Use native module declarations and matching paths. No `#[path]`, conditional path
  redirection, `include!` workaround, symlink workaround, or duplicated source trees.
- Tests use Given/When/Then and observable outcomes. Do not weaken them to get green.
  Behavior tests derive expected IDs, lengths and bytes from their case inputs;
  use schema layout for access setup. Keep independent literal wire/layout goldens
  in ABI tests. Drive hardware to an observable condition with a bounded watchdog,
  rather than asserting incidental step counts. Cases own inputs and expectations,
  harnesses assemble actors, and modeled software owns allocation and programming.
- No unsafe code, production `unwrap`/`expect`/`panic`/`todo`, or unchecked indexing.
- Keep Rust source below 250 pure LOC; split by responsibility.
- Organize modules by semantic ownership, not one type per file or line count alone.
  Keep closely related definitions and their implementations together; subordinate
  details belong under their owner, not beside it as unrelated top-level modules.
  Preserve cohesive units when satisfying the LOC limit; do not add forwarding
  layers or scatter implementations merely to make files shorter.
  Before completion, review changed modules and their neighbors for cohesion,
  dependency direction, unnecessary fragmentation, stale names/exports and domain
  responsibility leaks. Passing tests and satisfying LOC limits do not replace
  this architectural self-review; do not wait for the user to identify these issues.
- Use existing Cargo/rustfmt/clippy settings and terse public API docs.
- Do not stage build caches or incidental agent state.
- Prefix every git command with `GIT_MASTER=1`.

## Verification

Run from `model/modeling`:

```bash
cargo fmt --all -- --check
cargo test --workspace
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo doc --workspace --all-features --no-deps --document-private-items
```

The all-features pass exercises the generic C exporter and the current contract's
header CLI, consistency check, and strict C11 consumer. Export remains default-off.
