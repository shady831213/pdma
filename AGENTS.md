# PDMA REBUILD WORKSPACE

## Project direction

PDMA is a programmable copy engine for virtualized systems. Keep the direct
configuration usable without firmware. Managed allocation/lifecycle policy belongs
to firmware, Function/context enforcement to peripheral hardware, and memory
translation/requester identity to the integration platform. Current RTL work stays
DMA-core-only. Do not describe modeled isolation as complete SR-IOV, FLR, ATS/PRI
or logical-channel oversubscription support, and do not rename existing DMA APIs
merely to match the project tagline.

## Public repository policy

The GitHub repository became public on 2026-11-06 with explicit user authorization.
The public scope includes its existing branches, tags, history and commit identities.
The GitLab repository's visibility was not changed.

PDMA-authored RTL, model, firmware, software, tools, tests and documentation are
licensed under MIT OR Apache-2.0; see the repository-root licenses and README.
Third-party notices and terms remain applicable to their respective material.
The public repository retains the pinned Veryl compiler and checked-in
patches. Reference the [pcie_modeling repository](https://github.com/shady831213/pcie_modeling)
for their origins, not machine-local checkout paths. External upgrades do not
automatically change PDMA's toolchain. Any further changes to repository visibility,
history or commit-email disclosure require explicit user authorization.

History was sanitized on 2026-11-06 to replace machine-local reference paths.
Use the rewritten history on `github` and `origin`; do not merge pre-cleanup
archive branches into active branches. Push active changes explicitly to both
remotes. The local default push destination and active branch upstreams remain
GitHub. Do not use `--all` or `--mirror` to publish local archive refs. Author and
committer identities were preserved. Both remotes accepted the sanitized active
branches and checkpoint tag on 2026-11-06.
Force pushes are now disabled on both remotes. Use ordinary fast-forward pushes;
do not force-push or weaken branch protection without new explicit user authorization.
The post-rewrite check found that GitHub still serves old unreferenced commits
through its API. Clean reachable history is not proof of server-side erasure.
The user accepted the residual visibility of old machine-local reference paths
before authorizing publication; do not describe those old objects as erased.

## Current state

The user authorized a clean PDMA restart on 2026-09-09.
The previous implementation, including the last uncommitted memory refactor and
the retired sync model, is preserved on:

- Branch: `backup/pdma-before-rebuild-20260909`
- Commit: `51707fe1d38e261a188e1d0ac14243c0caf58127`

The replacement `contract`, `dma`, and `host` implement minimal statically configured channels.
The old implementations and `model/retired` remain only on the backup branch.
Do not restore their compatibility interfaces without a user request.

## Scope and retained material

The executable model is under `model/modeling`; read its `AGENTS.md` for model work.
RTL preparation is under `rtl`, `verification/dma`, `impl/dma` and `tools/rtl-flow`.
The first RTL phase is DMA only (confirmed 2026-10-29). CPU, peripheral
hw-components, firmware execution and subsystem integration wait until DMA RTL
converges. Keep their future interface boundaries, but do not implement them or
make their register redesign prerequisites for this phase.
`docs/rtl/architecture.md` records confirmed inputs and unresolved design choices;
it does not supersede the system specification under `model/modeling/docs`.
The DMA register-access slice has native tests: a 32/64-bit AXI4-Lite frontend,
reg/core CDC and object-owned registers. `DmaCopy` adds a minimal one-channel
Linear Copy and Fence32/64 slice with a fixed 32-bit register / 64-bit ordinary
memory profile, eight resident burst slots, independent read/write endpoints,
core/bus asynchronous queues, a reg/core mailbox and reset drain. Software
can submit Copy followed by Fence and poll its memory marker for completion.
It is not the complete
DMA execution/atomic/multichannel implementation. Aligned long-copy throughput
has its own zero-delay qualification; other profiles remain open. See
`docs/rtl/minimal-copy.md`. Register reset action/clear ports alone remain
integration boundaries, not evidence of transaction drain without that owner.
Follow `rtl/README.md` for source and verification ownership. PDMA independently
pins and maintains Veryl under `tools/rtl-flow`, starting from a verified snapshot
of [pcie_modeling's tool inputs](https://github.com/shady831213/pcie_modeling).
Do not modify that project's active toolchain or
automatically follow its updates; shared tool extraction is deferred.
The 2026-11-11 implementation target is 12nm with Genus: core and bus clocks
at 1GHz, register clock at 500MHz, still three asynchronous domains. ULVT is
forbidden; qualify library selection, mapping exclusions and actual mapped-cell
identities. Both CDC and RDC must use VC SpyGlass with PDMA-owned constraints
and reviewed findings. The Genus and VC SpyGlass flows are implemented;
the current mapped setup estimate meets the target with zero slack margin. The
redundant mailbox startup gates are removed; CDC/RDC reviewed gates accept only
source-bound, path-specific findings, including exact unconnected/no-clock sinks
with disabled consumers. Group counts never substitute for endpoint proof; raw
reports remain available. See `docs/rtl/throughput-review.md` for the current
proof limits and physical timing/MTBF obligations. Historical DC results and
randomized simulation do not substitute for these checks or physical signoff.
Genus and VC share generated clock SDC from the design profile. Instance-scoped
mailbox/FIFO constraints are reusable without creating platform clocks or global cuts;
fresh mapping validates the 0.5ns budgets and local synchronizer paths. See
`docs/rtl/constraints.md`; mapped timing does not close routed timing or MTBF.
Keep deployment, proprietary inputs and raw reports private; generic executable
flows and design constraints belong in the repository.
Veryl metadata v2 resolves package-owned constraint/checker resources into the
immutable RTL snapshot. Generic implementation adapters live under
`tools/rtl-flow/implementation`; `impl/dma` owns profiles and explicitly selected
source-bound reviews. Never load current workspace IP scripts to qualify an old
snapshot, and never apply waivers merely because a package is a dependency.
The model's Cargo workspace contains the core `contract`, `dma` and `host` crates, the
managed MVP's `hw-components` and `firmware`, shared PDMA `software`, system scenarios,
and six reusable building blocks:

- `hw-model-pipeline`
- `hw-model-swenv` (SoftwareEnv and main/ISR execution)
- `hw-model-io`
- `hw-model-irq`
- `hw-model-schema`
- `hw-model-schema-macros`

PDMA and the standalone downstream fixtures consume published `hw-model-*` crates
from crates.io. The library sources remain workspace members for maintenance;
they do not override registry dependencies. Preserve this distinction and do not
add local path dependencies or `[patch.crates-io]` overrides to the normal model build.

Schema 0.3.0 is published on crates.io and pins the unchanged schema-macros 0.2.0.
Its reviewed source commit is `6bd212bc53e21e994258a02d48e05f86af65b967`.
The temporary local schema override authorized for review on 2026-11-09 has ended;
PDMA and standalone fixtures consume the registry release with checked-in checksums.
Do not restore local path/patch overrides to the normal build.
The earlier paired schema/schema-macros 0.2.0 release maps to
`5a3a6f6cf13af855f3505968cfd21f601bfe83d7` in the sanitized history; its published
contents and checksums are unchanged. See `model/modeling/docs/publishing.md` for
release provenance and `model/modeling/docs/schema-export-review.md` for the review
record. Future releases require explicit user authorization.

The original scenarios sources are preserved on the backup branch only.
The new `crates/scenarios` contains current-model integration tests. Current acceptance
uses these system scenarios and the active crates' boundary tests.
The obsolete architecture specification and v0.x plans were removed from
`model/docs`; use Git history for those documents, not as current requirements.
Current architecture and reset documentation live under `model/modeling`.
`model/modeling/docs/firmware-dma-goal.md` records the next major direction,
not implemented behavior or an automatic implementation order.
`model/modeling/docs/firmware-dma-mvp.md` defines the approved architecture-validation
milestone before broader feature work. The implemented split is `dma`, `hw-components`
and `firmware`, with concrete wiring in `scenarios/tests/firmware_managed/harness`.
The standalone managed-runtime crate was removed after user review. Ordinary and
managed transports share the framework's split pending-interrupt capabilities.
DMA consumes IO memory capabilities; built-in error classification retains a concrete
memory error conversion. Its normal dependency tree has no Tokio. The software crate
owns SoftwareEnv, main/ISR execution, progress notifications and tracing. IO owns
memory/register access, supplied byte backing/allocation and public `memory::storage`
policies in one crate, with no schema dependency. Custom IO implementations remain
independent of the supplied backing types.
Hardware owns schema register windows and their effects. IO uses only std; no
separate memory or storage package remains. IRQ primitives are independent.
The managed assembly has independently firmware-managed channel windows, independent boot status,
and separate in-memory control command/response rings with bootstrap and bidirectional
doorbell registers. Host and firmware share `software::ring::SoftRing`; doorbell waits
are independent of DMA interrupt dispatch. Ready leaves DMA closed; Host requests
a channel and releases it through firmware, which serially awaits real DMA reset before
acknowledging reset or release. One PF management client pipelines bounded asynchronous
requests naming target Functions over one control ring pair. VF ports only operate granted
DMA data channels. Firmware owns Function-to-channel
allocation; peripheral registers enforce MMIO ownership and route channel notifications.
See `model/modeling/docs/{control-channel,function-isolation}.md`. Managed DMA can
use independent Function memory capabilities or an explicitly shared backend.
DMA tags accesses with channel/context selectors; hw-components resolves owner
registers and optionally an instance-sized context table. Context attributes are
an extensible schema record, initially Default/PASID (Default differs from PASID zero).
Runtime selects injected Function/address-space backing; allocation stays platform-owned.
PF control memory has its own enable-frozen configuration, not a reserved table slot.
Firmware supports Create/Drain/Abort/DestroyContext through its existing control queues.
Hardware accepts an atomic context-ID plus Drain/Abort request; firmware implements
Destroy by awaiting Drain success and explicitly clearing valid. A retained failed-channel
bitmap is the final context-map bank and covers all detected failures in the operation scope.
DMA captures producer frontiers, discards unstarted or parked Abort targets and retains
accepted task references through suspension until terminal retirement. Running Abort
targets remain command-atomic unless they park. Drain encountering parked work returns
ChannelStopped with the retained failure bitmap. Failure retains the binding
and permits channel-reset recovery. See model/modeling/docs/context-contract.md for
mandatory software lifetime/submission obligations. No PASID/IOMMU translation,
Function reset/FLR or PCIe integration is implemented.
Managed firmware handles DMA fatal notifications, closes work windows and publishes
PF events over an independent in-memory event ring and event doorbell. Event failure
is retained in PF-readable firmware-owned status without retries or backpressure.
ResetDma preserves grants/bindings and management state but leaves work windows closed;
ResetEvents rearms only a failed event producer. See docs/spec.md and docs/fatal-events.md
under model/modeling for system requirements, including firmware and RTL obligations.
PF-only firmware management retains per-Function work MMIO and IRQ isolation.
Hardware checks the platform-selected PF identity on every control-register access;
firmware validates request targets and channel ownership. The agreed future VF FLR
contract requires PF to release all target channels successfully before FLR and
prevent reacquisition until FLR completes. No firmware FLR ingress or repeated
cleanup is needed: application ACK may be immediate under that precondition, while
PCIe drain/controller handshake stays at integration. Direct FLR without preparation
is unsupported, with no automatic check or recovery. PF FLR is a separate scope;
FLR remains unimplemented. See `model/modeling/docs/function-isolation.md`.
The generated header under `model/modeling/include` describes the current contract.
`docs/white_boards/**` remains exploratory background.

The current implementation has an instance-configured channel count with independent reset,
command/error-event rings and one shared DMA execution pipeline. Copy retains independently
selected linear/scatter-gather endpoints. Write, Fill, Wait, Atomic, Semaphore, Fence,
Trap and Timestamp share the same accepted ownership and reset boundaries. Successful commands
produce no event; software observes Fence storage and explicit Trap notifications.
Atomic32/64 capabilities are immutable instance configuration, defaulting off in
ordinary constructors. Atomic has no old-value destination. Ordinary false Wait and
Semaphore Wait release the shared grant without setting fault status; same-channel
successors remain blocked. Stopping errors latch before event I/O; event-full,
event-write and event-configuration failures are fatal, never admission backpressure.
Trap has an independent sticky cause and write-one acknowledgement; masks never replay.
Recoverable payload PageFault parks the original command and both Copy
cursors, including successful read scratch for a failed write, while peers can run.
Owner software repairs through its platform, observes FAULT_STATUS.faulted and
writes RESUME.request=1 in channel work MMIO; both registers are 32 bits. No fault
event or DMA IRQ is produced. Software must serialize repair/resume with
resource lifecycle operations. Platform-defined opaque tags distinguish fault
sources even when channels share a Function/PASID; the MMU echoes tags rather than
interpreting PDMA channel IDs. hw-components exposes authorized source/identity
sideband mapping; MemoryIdentity stays address-space-only. Tags must retain their
association through outstanding reports/handlers. Software must coordinate
reset, release and reallocation and retire old fault handling before reuse; hardware
does not correlate late or duplicate requests with fault episodes. Error-only Event
records have no Success or IOC fields. Fence and Timestamp target PageFaults park
like ordinary command writes; permanent failures stop with WriteError. Timestamp
samples an injected external timer once and retains that value through write faults.
The harness owns time progression; neither sampling nor DMA reset advances/resets it.
See `model/modeling/docs/copy-engine.md` and
`model/modeling/docs/page-fault-recovery.md`. No true MMU, PCIe integration,
oversubscription or global barrier semantics are added. Do not expand that scope or
recreate the old API surface to satisfy historical tests.

## Working rules

The 2026-10-21 decision withdrew the first Signal/Wait feature. The approved
2026-10-22 copy-engine command families replace that design while retaining generic
memory atomics and reset during pending context maintenance. Use
`model/modeling/docs/copy-engine.md` for the current command contract and the
copy-engine reassessment in `model/modeling/docs/firmware-dma-goal.md` for future
decisions. HSA is a semantic reference; AQL compatibility is not a current goal.

- Use native Rust module paths. `#[path]` and conditional path redirection are forbidden;
  do not bypass this with `include!`, symlinks, or duplicate source trees.
- Preserve retained building-block behavior and tests.
- Keep PDMA wire/protocol rules in `contract` and shared software algorithms in
  `software`. An `Entry` type parameter alone does not justify framework placement.
  Generic schema/runtime crates expose no PDMA ring protocol or ring types and have
  no normal dependency on PDMA crates; transport, CPU and notification mechanics
  remain in the generic runtime.
- Add failing behavioral tests before new model implementation.
- Keep hardware behavior deterministic and step-based, without cycle timing.
- Hardware never stores or invokes software interrupt handlers.
- Keep explicit CPU-local resource and interrupt-dispatch safety guarantees intact.
- Do not impose CPU affinity on ordinary drivers or rings. CPU-specific resource
  checks and main/ISR scheduling belong to runtime; queue ownership is not CPU identity.
- Keep memory ownership and backend selection outside hardware behavior.
- Keep Rust files below 250 pure LOC and use the workspace lint configuration.
- Do not add unsafe code or weaken tests to pass validation.
- Do not stage build caches or incidental agent state.
- Do not add local-machine details or commercial EDA deployment information to
  tracked files: server identities, installation paths, license settings,
  proprietary technology/library inputs, logs and reports stay outside this
  repository. This restriction concerns commercial EDA tools, not public
  open-source tooling. Authorized Veryl versions, patches, generic launchers and
  public provenance may be maintained here without machine-specific settings.
  Reproducible commercial-tool invocation, verification/regression, synthesis,
  generic constraints and report checks also belong in the repository. Inject
  deployment settings through arguments/environment or a trusted private config;
  see `tools/rtl-flow/flows.md`. Do not move reusable flow code out merely because
  it invokes a commercial tool.
  Check the staged diff against this restriction before each commit.
- Use `GIT_MASTER=1` before every git command.
- Use short English commit messages matching repository history.

## RTL microarchitecture working principles

Reusable RTL belongs in `rtl/common_cell`, currently snapshotted from pcie_modeling
with source identity and tests. Use its existing cells instead of duplicating
transport/storage/selection logic. Technology-specific implementations stay behind
the common contracts; `rtl/tech_cell/generic` contains only generic RTL providers.
Keep DMA/PCIe protocols and lifecycle policies outside common_cell. Shared-repo
extraction is deferred; do not add machine-local source references to Git.

Adapted from [pcie_modeling](https://github.com/shady831213/pcie_modeling).
These principles apply to the DMA-core scope; they do not import that project's
topology, technology targets, tool deployment or implementation results.

- Keep the functional model semantic. Feed missing ownership, ordering and
  lifecycle contracts back into the specification/model; do not make it mirror
  RTL RAM ports, muxes or exact cycle counts.
- Describe hardware top down: module responsibilities, state lifetimes, register
  cuts, storage access, hazards, timing paths and tradeoffs. Keep experiment
  chronology and evidence separate from the architecture description.
- Make allocation, binding and release physical: selection, handshakes, row write
  enables, identity retention and the terminal event that permits reuse. Define
  simultaneous-event priority and when released capacity becomes visible.
- Array syntax does not prove a memory implementation. Account for storage,
  read/write ports, arbitration, mux depth, fanout and latency. Keep per-entry
  state and update priority cohesive; place allocation and cross-entry decisions
  at their actual shared owner. Do not split storage merely to simplify code.
- Use native interfaces/modports and typed payloads. Flatten only at real
  integration boundaries; adapters must not silently add storage or latency.
- Express internal invariants as executable assertions, not synthesized sticky
  error monitors. Preserve actual protocol, admission and runtime error behavior.
- Verify the real DUT against an independent oracle. Check identity, causal
  order and delivery skew, including no early completion or lost ownership;
  avoid imposing accidental same-cycle behavior or designing around test probes.
- Compare performance under matched source/configuration and constraints. Do not
  move cost outside the measured boundary or weaken throughput requirements to
  claim improvement. Elaboration success is not physical feasibility.
- Iterate with small and representative configurations and explicit scaling
  analysis. Validate maximum configurations at stable milestones; keep required
  final coverage. Run focused regressions first, then affected end-to-end paths.
- Keep compiler crashes distinct from expected runtime failures. Zero selected
  tests, retry-only passes or weakened assertions are not qualification evidence.
- Reuse immutable build artifacts only when all relevant input identities match;
  isolate outputs and distinguish reuse from fresh builds. Measure build/runtime
  costs without reducing verification to improve reported turnaround.
- Replace an unsuitable architecture rather than accumulating wrappers. Reuse
  mature primitives and remove obsolete paths/tests/dependencies; history is the
  archive. Distinguish sourced hardware facts, deductions and hypotheses, and do
  not assume a model's transaction identifiers require a central RTL table.
