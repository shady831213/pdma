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

`include/pdma_model_abi.h` is generated from the current contract schemas by
`pdma-abi-header` with the optional `export-c-header` feature. Regenerate and check
it using the commands in `include/README.md`; never edit it by hand. The old ABI
header is preserved on the backup branch, not in the active include directory.

The documents under `model/docs`, including the previous architecture specification,
describe the backed-up implementation. They must not force old software interface
shapes into the rebuild.

## Implementation constraints

- The replacement models statically configured channels, command/response rings, linear/SG DMA copy,
  independently masked non-empty/IOC notifications, and channel/device error/reset boundaries.
  Do not implement further slices without a user request.
- The constructor fixes the channel count for each instance; software only reads it.
  Contract layout/export takes that count explicitly, with generated child register blocks.
  One shared execution pipeline processes whole commands selected by round-robin arbitration.
  Use Arbitrate (the singleton specialization of FanIn) over channel-bound Fetch ports,
  with the RoundRobin policy owned by device. Ready checks do not consume commands,
  and PDMA eligibility rules belong to Fetch, not the generic combinators.
  Route tagged responses through Dispatch (the singleton FanOut specialization) to
  channel-bound publication ports.
  Complete hardware assembly uses Device::command_loop(); scenarios drive one retained
  operation and abort/join it after software finishes. Do not recreate command-local
  completion/reset/fault recovery in a scenario-side loop. Single-command boundary
  tests may assemble existing stages in test fixtures, not a production factory.
  Skip full-response channels before accepting commands; do not add per-channel copy state.
  Bind accepted work to channel identity and generation so reset invalidates only its work
  before the next side effect, including after subsequent reconfiguration.
  Follow the request/cleanup/completion contract in `docs/reset.md`: reset write one
  requests cleanup; read one means busy; only hardware clears zero after affected accepted
  work retires. Zero writes do nothing and repeated requests coalesce. Reject configuration
  writes in a busy scope with `Error::ResetInProgress`; retain read access.
  Use `AfterEach<Repeat<RunCommand>, ResetControl>` for the command loop. Track accepted
  channel/generation ownership until its terminal release; generation invalidation alone
  is not completion. `RunCommand` is the sole ownership-release boundary when the
  complete pipeline terminates, including normal completion, reset and errors.
  Response publication and `Resettable` binding must not retire the whole command.
  Custom command assemblies must retain this lifecycle boundary and drive `ResetControl`.
  Guard command, SG-list, payload and response accesses before old work can interact again.
  Memory calls are synchronous within a step: do not invent bus transactions, outstanding
  queues, cycle timing or timeout delays. Host reset waits for hardware completion; caller
  cancellation does not release affected memory or cancel hardware cleanup.
- Source and destination independently select linear spans or SG lists. Software owns
  list/buffer allocation and keeps descriptors stable until completion. Hardware checks
  total lengths before payload writes, reads lists incrementally and retains cursors;
  do not cache whole lists, reject overlap or invent segment-count/total-length limits.
  Fixed copy scratch size is an implementation detail, not a command-size limit.
- Reuse FanIn/Arbitrate/FanOut/Dispatch/Then/Repeat/AfterEach for execution composition. Runtime supplies memory and
  driving only; it must not gain PDMA traversal, validation or copy policy.
- Queue state and its registers belong to the hardware object; actions compose through
  existing stages. Runtime assembly selects memory backend and execution policy.
- The shared `Ring` trait's entry geometry and pointer rules belong to contract. Host/device
  keep side-specific memory/MMIO access and publish pointers only after successful
  entry access. Do not introduce a shadow cursor store or duplicate ring arithmetic.
- Keep the model deterministic and step-based. No cycle timing or extra executors.
- Device notification masks never replay old events when cleared. Platform-pending
  notifications survive device masking/reset until claimed. Response errors stop
  the channel after publication; reset never rolls back external memory writes.
- Software handlers remain outside the hardware. Preserve CPU-local and handler
  execution guarantees supplied by the runtime.
- Use native module declarations and matching paths. No `#[path]`, conditional path
  redirection, `include!` workaround, symlink workaround, or duplicated source trees.
- Tests use Given/When/Then and observable outcomes. Do not weaken them to get green.
- No unsafe code, production `unwrap`/`expect`/`panic`/`todo`, or unchecked indexing.
- Keep Rust source below 250 pure LOC; split by responsibility.
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
