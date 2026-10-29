# PDMA MODELING WORKSPACE

## Current state

This workspace is the starting point for a clean PDMA rebuild.
The old contract, device, host, and retired sync implementation were deliberately
removed from master after backing up the complete source snapshot:
`backup/pdma-before-rebuild-20260909` at
`51707fe1d38e261a188e1d0ac14243c0caf58127`.

The replacement implements instance-configured channels with command and error-event rings,
Copy/Write/Fill/Wait/Atomic/Semaphore/Fence/Trap/Timestamp, error/reset boundaries, and event/Trap IRQ delivery. See README.md
and the `crates/scenarios/tests/{single_channel,multi_channel}/main.rs` system tests. Old interfaces, numeric register
layouts, and ABI compatibility are not requirements for this model.

## Active building blocks

| Crate | Responsibility |
| --- | --- |
| hw-model-pipeline | Typed stages, generic gathering/distribution, composition, and driving policies. |
| hw-model-swenv | SoftwareEnv, environment-local state, main/ISR execution, progress hints and tracing. |
| hw-model-io | Memory/register interfaces, supplied byte backing/allocation, shared-state storage and observation. |
| hw-model-irq | Source/claim/handler capabilities and optional Tokio pending-interrupt backend. |
| hw-model-schema | Runtime-neutral record/register schemas, metadata, access, and exporters. |
| hw-model-schema-macros | Schema code generation and compile-time validation. |

Keep these crates and their tests intact. Reuse their existing capabilities in the
replacement model; do not introduce parallel composition or dispatch mechanisms.

All `hw-model-*` dependency declarations use crates.io versions, including library
sibling dependencies and standalone fixtures. Local library workspace members are
maintained and tested separately from the registry packages consumed by PDMA.
Do not add path/patch overrides to the default build. Verify resolved source IDs
from PDMA roots when changing dependency wiring; names alone cannot distinguish
local members from published packages with the same name and version.

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
channel driver; generic CPU/interrupt mechanics stay in `hw-model-swenv`.
`EnvOwner` is runtime-private. Software entrypoints and ordinary drivers must not
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
Context maintenance retains one pending request while firmware can accept ResetChannel,
ResetDma and ResetEvents. Other requests arriving while it remains pending receive
ContextBusy; response IDs, not response order, identify requests. An affected reset
fails the Drain without discarding accepted/resident cleanup tracking. An unrelated
channel reset does not cancel the Drain. Reset success still waits for actual cleanup.
Host must continue consuming responses and preserve an outstanding slot for recovery.
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
register bases and validates disjoint apertures. PF control-response backpressure stops management request admission;
DMA event capacity never gates work admission. Do not turn simulation
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

DMA and hw-components consume IO memory capabilities rather than software execution.
Built-in backing error classification is explicit; neither normal dependency tree may
acquire Tokio through workspace feature unification. IO uses only std; local observation
owns its Rc/RefCell state without using the supplied memory model. IO's `memory`
module contains both access contracts and supplied backing, plus `memory::storage`
for closure-scoped generic state. No separate memory/storage package, schema feature
or register-image abstraction is needed. Hardware composes schema register windows
and owns access side effects. Custom MemoryAccess implementations need not use a
supplied backing, allocator or storage policy.
IRQ traits work without Tokio or software-environment scheduling; its optional `tokio`
feature supplies pending interrupts. The software crate owns SoftwareEnv/main/ISR
execution and depends on IO and IRQ traits, without schema normal dependencies.
Do not restore moved API reexports as a compatibility umbrella. EnvOwner and
synchronization guards remain private.
Both concrete transports use `interrupt_wire`: hardware-side adapters hold the
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

- The replacement models statically configured channels, command/error-event rings,
  Copy/Write/Fill/Wait/Atomic/Semaphore/Fence/Trap/Timestamp and channel/device error/reset boundaries.
  Success writes no Event. Fence publishes completion storage; Trap has an independent
  sticky IRQ cause and write-one request acknowledgement. Event-non-empty and Trap masks
  never replay; error/fatal notifications bypass those masks and obey irq_enable.
  DmaConfig selects immutable atomic32/atomic64 capabilities; ordinary constructors default off.
  Atomic has no old-value output. Semaphore counts reject Signal overflow before mutation.
  False Wait/Semaphore Wait yields the grant without a fault bit or RESUME obligation;
  same-channel successors remain blocked, peers run, Drain waits and Abort may cancel it.
  Fence and Timestamp target PageFaults use the ordinary retained write recovery;
  permanent failures remain stopping WriteError. Timestamp retains one sample per
  accepted command, including zero, and never resamples on resume. Timer is a
  read-only external input, attached by the assembler with Dma::with_timer; absent
  input reports Unsupported. Its concrete counter lives in the harness, not DMA or
  the generic runtime. Harness ticks are not RTL cycle or performance predictions.
  See docs/copy-engine.md for width, ordering and lifetime obligations.
  Do not implement further slices without a user request.
- The constructor fixes the channel count for each instance; software only reads it.
  Contract layout/export takes that count explicitly, with generated child register blocks.
  Register banks repeat complete child blocks; their spacing derives from child extent
  and alignment, with no placement stride override. Reserved gaps belong to their
  enclosing block and cannot host sibling placements or interleaved banks. Group
  related registers in one child block before repeating it.
  Channel is a persistent hardware state object in the shared Dma context, not a
  pipeline. Its private task state owns the accepted descriptor ledger, validated
  traversal and concrete read/write fault checkpoints. Accepted lifetime persists
  even while a stage owns validation, payload or event data. These states must
  describe hardware registers/storage, never Rust S::State, ThenState, Operation
  or a saved generic continuation. Local validation and current payload travel in
  stage input/output/state; they are not a shared Action program counter.
  State representation and local operations belong under channel; shared stage
  implementations belong under stages. DMA-wide Work identity,
  guarded access and retirement helpers belong in the DMA-level work module.
  Do not place shared execution stages under channel to accommodate visibility.
  The data path composes command admission through FanIn using channel-targeted Candidate
  inputs and Arbitrate::snapshots with DMA's RoundRobin selector. Candidates are task
  requests, not channel selections: reject stale head/generation/retained-task
  position rather than substituting another task. An active non-waiting command stays inside
  its execution continuation and backpressures all candidates. Fresh selection
  enters Fetch as the opcode FanOut's input; a resumed request grants the channel without
  fetching a new descriptor. Fetch outputs the accepted descriptor with explicit
  fresh/resumed origin. Dispatch selects the opcode branch once, without a second
  Copy/Operation family or a ResolveCommand classification stage. The Copy branch alone owns CheckRanges and Install,
  which forwards the new or retained chunk request into its execution graph. RoundRobin implements
  Arbitration, not FanPolicy; snapshot disposal belongs to the generic adapter.
  Candidates emit non-owning observations and do not consume descriptors. The
  selected work is revalidated before acceptance. Descriptor and execution stages use a freshly borrowed ExecutionContext resource bank, selecting
  narrow channel resources by each Flow's identity, not all of Dma. Typed command, chunk,
  payload and completion values carry actual data plus private identity metadata.
  Do not substitute renamed Work wrappers for dataflow or copy mutable hardware
  state into a second context. The borrowed context never escapes a step.
  No hardware leaf invokes another stage. Reuse Then for dataflow and FanOut with
  Dispatch for actual opcode branches, not for channel-owned event pushes.
  Error-origin stages invoke the selected Channel's guarded report operation;
  there is no Events/Publish pipeline or event-merging tail. Do not add CommandScope, a second engine,
  a centralized step-dispatch switch or a wrapper that selects child execution phases.
  A stateless context projection may only borrow the execution resource bank and
  delegate step. It must not store task identity, observe completion for cleanup,
  interpret checkpoints, synthesize ThenState or step nested Then children.
  Dma::hardware() returns one connected pipeline driven by HardwareDriver and
  AsyncExecutor::drive. Scenarios abort/join after software finishes. No group
  driver, independent lifecycle-stage tuple or runtime lifecycle policy is needed.
  Hardware contains FanIn with the real accepting/executing subgraph as its
  output and a local terminal-error boundary, not SelectedRequest/Grant adapters or
  sibling-unit state or lifecycle coordination. Its finite invocation drains
  before the external driver restarts admission; no old faulted path may consume
  a newly reacquired grant. Do not recreate Service, separate admission/execution
  round barriers or a global phase selector under another name.
  Do not recreate command-local
  completion/reset/fault recovery in a scenario-side loop. Single-command boundary
  tests may assemble existing stages in test fixtures, not a production factory.
  Never gate command admission on event capacity. Event-full/configuration/write failure
  is fatal only when reporting a stopping error; latch stop before attempting event I/O.
  Preserve per-channel parked
  command/cursor/scratch state without adding engines or physical-channel oversubscription.
  Bind accepted work to channel identity and generation so reset invalidates only its work
  before the next side effect, including after subsequent reconfiguration.
  Follow the request/cleanup/completion contract in `docs/reset.md`: reset write one
  requests cleanup; read one means busy; only hardware clears zero after affected accepted
  work retires. Zero writes do nothing and repeated requests coalesce. Reject configuration
  writes in a busy scope with `Error::ResetInProgress`; retain read access.
  No minimum busy duration or extra model step is part of the software contract.
  Idle/parked scopes without resident pipeline state can complete on request after
  cleanup. Each resident stage must observe reset in step and release its own local
  work before completing, independently of data readiness. Fetch owns admission and
  residency acquisition; Retire consumes the FanOut collection of explicit Flow
  results to release it. Then connects branch assembly directly to Retire; do not
  restore an Opcodes stage solely to unwrap a singleton collection.
  Accepted execution uses Error=Infallible; domain stops travel as Flow data so Then
  cannot bypass retirement. Preserve raw driving errors as unfinished continuations.
  Do not restore Selected or move its lifecycle observation into Hardware. Track resident identity
  separately from accepted ownership and active grant. Parking releases the grant;
  error reporting keeps ownership/residency until the common Retire boundary. Do not clear the
  ledger and acknowledge reset while leaving a live old invocation to finish later.
  IRQ causes update in source-owning operations. Context maintenance observes owner
  transitions; descriptor reads/discards belong to Fetch. No global tick sweep,
  per-child-step AfterEach replacement or runtime cleanup policy is allowed.
  Track accepted channel/generation ownership until real hardware work retires;
  generation invalidation alone is not completion. A payload fault completes this
  finite execution path while retaining concrete Channel state and releasing the
  shared grant. Resume permits a new grant; the read and write stages consume
  their own retained checkpoints without refetching, restarting validation or
  repeating completed reads and valid-data transforms. A failed write retains
  transformed scratch at the write boundary, not a next-stage instruction.
  A healthy non-waiting command retains hardware state, residency and its grant in one execution
  invocation. CommandExecution advances its opaque chunk graph once per step;
  a completed intermediate chunk supplies the next typed input from retained
  hardware traversal without returning to admission, arbitration or validation.
  The composite does not inspect child phases or duplicate command progress.
  Empty candidate sweeps cannot advance RR state. A fault releases
  the grant; even immediate RESUME requires new arbitration before more payload work.
  Reset/Abort cleanup must progress despite a disabled peer.
  Opcode dispatch routes the explicit Fetch output before Copy-specific validation;
  resumed Copy bypasses fresh validation and preserves installed traversal/scratch.
  Branches project their operands on acceptance, not by re-dispatching the entire
  command on each chunk. Each branch returns Flow<Completion>; FanOut collects
  Vec<(branch index, Flow<Completion>)> for the same Retire boundary. Retire processes
  every supplied receipt even after an earlier error and returns the first error
  in input order. Branch indices never replace Flow owner identity. Empty collections
  complete without effects. No generic Fold or Fan context/update extension is needed.
  Dma::hardware_with accepts a Copy execution graph from Flow<ChunkInput> to
  Flow<Completion> with an Infallible domain-error port over ExecutionContext.
  Put chunk preparation/read/transform/write inside CommandExecution; custom
  command tails compose after it and preserve its typed result. Do not compose
  event publication into either execution branch. Completion is terminal success,
  not an optional Event transport; error origins report once through Channel and
  propagate a stopped Flow. Waiting/parked paths carry no completion and keep
  accepted ownership after residency is released. Retire also clears accepted work
  and the grant for terminal custom errors, with exact identity/generation guards.
  A chunk graph maps Flow<ChunkInput> to Flow<Completion> and reports real command
  progress. Flow is DMA-specific data and ownership metadata, not a generic rule;
  ordinary Stage output can still be unit. Custom stages use Flow::map/stop/finish
  to preserve ownership and Flow::lifecycle before effects/readiness checks.
  There are no destructor-based retirement or manual per-stage ticket releases.
  Dropping a Flow is not retirement. Fetch rejects replacement of resident work,
  including after error notification or parking.
  Do not require a specific nested Then shape or extract child hardware from a
  caller's graph. Stopping errors never fabricate payload data, and downstream
  propagation never reports the same error again. A write-fault
  retry supplies retained payload locally to the same WriteCopy hardware object;
  upstream stages may see None bubbles but must not reprocess retained bytes.
  Do not use a separate hardcoded retry pipeline or top-level error-to-stage routing.
  Fresh validation runs once; later chunks use installed hardware traversal.
  Stage outputs and payload geometry are owned, non-clonable and not publicly
  forgeable. Transforms may modify the bounded payload bytes, not addresses/length.
  Persistent custom data must have explicit hardware ownership; arbitrary Rust
  child-state destructor ordering is not a reset or accepted-lifetime contract.
  Guard command, SG-list, payload and event accesses before old work can interact again.
  Memory calls are synchronous within a step: do not invent bus transactions, outstanding
  queues, cycle timing or timeout delays. Host reset waits for hardware completion; caller
  cancellation does not release affected memory or cancel hardware cleanup.
- Source and destination independently select linear spans or SG lists. Software owns
  list/buffer allocation and keeps descriptors stable until completion. Hardware checks
  both selected ranges before payload writes, reads lists from explicit start positions
  incrementally and retains only current cursors. One command copy_length selects
  independent Linear byte offsets or SG entry/local offsets; backing totals need not match;
  do not cache whole lists, reject overlap or invent segment-count/total-length limits.
  Overlap is software-owned (HOST-CE-5): hardware does not check it, but no portable
  within-command overlap result is promised. The deterministic overlapping-SG test
  characterizes this model, not an RTL access-order requirement. Preserve range,
  authorization, fault-retention and cleanup guarantees even for overlapping input.
  Fixed copy scratch size is an implementation detail, not a command-size limit.
- Reuse FanIn/Arbitrate/FanOut/Dispatch/Then for execution composition.
  Repeat and AfterEach are removed. Do not recreate them with renamed combinators,
  ControlFlow feedback, per-child-step hooks or generic repetition wrappers.
  The approved DMA-local CommandExecution is the narrow exception to the former
  blanket child-reuse prohibition: it feeds the next chunk into the same completed
  child graph while retaining command ownership, never selecting child phases or
  restarting an unfinished child. Do not add a generic Producer/Stream for this.
  Each stage handles input/output and local state; persistent
  cross-stage state has an explicit hardware owner in context. Combinators connect
  values and express routing/backpressure, not device task lifecycle.
  Express hardware feedback through
  typed input/output and shared state; continuous top-level driving belongs to executors.
  Runtime supplies memory and
  driving only; it must not gain PDMA traversal, validation or copy policy.
- Queue state and its registers belong to the hardware object; actions compose through
  existing stages. Runtime assembly selects memory backend and execution policy.
- The shared `Ring`, `RingPointers` and `RingError` definitions live in `contract`;
  PDMA entry geometry and pointer rules are protocol, even with an `Entry` parameter.
  `software::ring` owns `RingConfig`, `SoftRing` and their cursor I/O adapters;
  host retains its `RingConfig` reexport. Host and firmware share this PDMA software
  crate, whose normal dependencies include contract, IO, software execution and schema, never host, firmware
  or DMA. Its `dma` module also owns the shared queue driver and register-only
  `ChannelControl`; firmware uses the latter for reset without owning DMA rings.
  Generic libraries expose no ring protocol or ring types and have no normal dependency
  on PDMA crates. Transport stays in IO; software-environment execution and progress
  hints stay in the software library. `SoftRing` supplies exclusively borrowed software operations with
  borrowed MMIO or memory cursors. Host/device
  keep side-specific memory/MMIO access and publish pointers only after successful
  entry access. Do not introduce a shadow cursor store or duplicate ring arithmetic.
- Keep the model deterministic and step-based. No cycle timing or extra executors.
  Stage<C> exposes Input/Output/Error, not State or start. step takes a mutable
  Option<Input> slot: take transfers ownership, busy stages leave new input untouched,
  and accepted progress lives inside the stage. None means no offered input;
  Some(None) is a domain bubble for optional inputs. Stage itself is the continuation;
  no external flush/cancel hook or exposed private state is allowed. Domain reset
  belongs to context and is observed by stages in step. A call error preserves the
  continuation, rather than proving hardware completion. Fan error handling stops
  new admissions and waits for accepted children to complete through step before
  reporting its retained error. Stage completion releases invocation-local intermediate state and transfers its
  output; InProgress retains unfinished work. Async hardware driving yields after
  every bounded step, including idle polling. Do not restore Pending, wait tokens,
  or ReadyStage. Software interrupt/doorbell waits remain event-driven. Generic Fan
  advances independent inputs, plans only over completed values, and advances
  selected outputs in separate phases; retain unconsumed values and unfinished
  states until consumed or terminated. No engine/stage object belongs in context.
- Device notification masks never replay old events when cleared. Platform-pending
  notifications survive device masking/reset until claimed. Stopping errors halt
  the channel before event I/O; reset never rolls back external memory writes.
  Payload PageFault parks the original command without a event or DMA IRQ/event.
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
  bitmap; Abort discards matching parked work without event, including work that
  parks after Abort acceptance. Metadata faults retain their stopping/fatal behavior.
  Event is error-only, with no Success or IOC fields; payload PageFault is not an event code.
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
