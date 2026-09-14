# Hardware interface pipeline

`hardware-interface-pipeline` is a workspace-private, runtime-neutral algebra for
statically composed hardware interactions. It depends on no storage runtime, lock,
async runtime, or device model.

`Stage<C>` describes reusable hardware with explicit `Input`, `Output`, and per-request
`State` types. `start(input)` creates request state without accessing hardware or context;
`step(&mut self, &mut C, &mut State)` performs at most one architecture-visible interaction.
`Operation<S, C>::new(input)` owns that state independently of the hardware and driving
future. `StepOutcome` transfers owned wait, output, and error values as `Progressed`,
`Pending`, `Complete`, or `Exhausted`. Completion or reset exhausts the operation; the
same hardware can accept another operation. Dropping request state preserves effects
already committed to the context or hardware.

The `src/combinators/` module groups `Then`, `AfterEach`, `Repeat`, `FanIn`, `Arbitrate`,
`FanOut`, `Dispatch`, and `MapError` in native modules. Their types remain re-exported
from the crate root; execution policies live separately under `src/executor/`.
The `fan_in/` module owns `FanIn` and its `Arbitrate` specialization;
`fan_out/` owns `FanOut` and its `Dispatch` specialization.

`Then<A, B>` owns both hardware stages and requires `A::Output = B::Input`, with matching
error and wait types. `upstream.then(downstream)` connects existing hardware. Each
operation transfers an upstream result into downstream state exactly once and
returns `Progressed` without stepping downstream. Every stage handoff has the same
boundary; `Pending` and `Progressed` are never retried in the same call.
A downstream `Pending` resumes retained state without replaying upstream work.
Public `upstream` and `downstream` fields keep
hardware configuration and counters accessible between operations.

`FanIn<S, P>` owns source stages and an application `Combine` policy. The policy
retains operation state, plans an ordered subset of inputs, and reduces their owned
outputs to its own output type. An empty plan reaches the reducer with no outputs.
`Arbitrate<S, A>` specializes this same lifecycle with an application `Arbitration`
policy, a singleton plan, and identity reduction. Round-robin, priority, and idle
behavior belong to applications. `ReadyStage` exposes non-consuming eligibility;
`Arbitrate` forwards readiness for nested selection. It rechecks the chosen source
before its first step. If readiness disappeared, it discards that unstarted selection
and plans again, preserving the arbitration policy's cursor. After the first source
step, retained work is never discarded because readiness changed.

`FanOut<S, P>` accepts an application input and uses `Distribute` to create an ordered
plan of owned requests and reduce their results. Policies choose how to split,
move, or explicitly clone their input. Fan-in and fan-out share one branch stage:
it validates every index and duplicate before any child starts, then steps at most
one child per outer step. Planning, branch execution and reduction run in separate steps.
Pending work blocks later branches and retains completed outputs without cloning.
Child failure or exhaustion terminalizes the whole operation. `BranchError` separates
invalid plans from application errors; `MapError` adapts error types when connecting
stages with `Then`. These combinators use the existing operation and executor APIs.

`Dispatch<S, R>` specializes `FanOut` for one destination and a unit completion result.
An application implements only `Route::Input` and `Route::route`, returning the selected
index and its owned input. The private adapter creates a singleton plan and discards
the successful receipt; applications need no operation metadata or `combine` method.
Routing runs once per operation. Index validation, pending work, errors and completion
boundaries are inherited from `FanOut`, without a separate execution state machine.

The private `combinators/fan/` module owns the shared port population, policy borrowing,
composition state and assembly for both fan combinators. Their public wrappers supply
`Plan` or `Split` and the application reducer to the same
`Then<PlanOrSplit, Then<Branches, Reduce>>` composition.
`ThenState` owns their phase transitions. `Branches` is `Repeat<RunBranch>`:
one iteration advances one selected child, returning the remaining plan and accumulated
outputs through `Continue`, or the final aggregate through `Break`. `Repeat` owns iteration
and termination; a pending child keeps its current state. Preparing an iteration does not
start its child, so arbitration can still invalidate an unstarted selection.
`Branches` and `Reduce` are internal stages in that module, not application-facing combinators.
Both fans use the same `Reduce` stage, supplying their policy's reduction function and consuming
the owned metadata and outputs once. The policy and connected
hardware remain owned by the fan combinator; stateless internal stages borrow them and
the application context for each call. No frame borrow enters retained operation state.
`Then` and `AfterEach` forward readiness from their upstream source, so complete
source pipelines can be combined without running transforms or updates during selection.

The executors select interaction drive policy, not device behavior:

- `StepExecutor::step(&mut hardware, &mut operation, &mut context)` advances one leaf step.
- `ImmediateExecutor::run(&mut hardware, &mut operation, &mut context)` loops across
  `Progressed` outcomes within a nonzero budget and returns an owned wait token, terminal
  result, or executor error.
- `AsyncExecutor<B>::run(&mut hardware, &mut operation)` asks an
  `AsyncInteractionBackend` for one closure-scoped context interaction per leaf step,
  then awaits an owned wait token after the closure ends.

The backend may await before or after the synchronous interaction closure, but no
context borrow or guard can cross that boundary. Consumers keep heterogeneous device
pipelines in their own statically dispatched enums; the crate has no `dyn Stage`, boxed
future, dynamic graph, stage registry, retry policy, or Tokio dependency.

This is the interaction-drive layer in the reusable stack. Schema descriptors and
borrowed views belong to `hardware-interface-schema`; storage and generic bus/memory/
observation algorithms belong to `hardware-interface-runtime`; operation, reset, IRQ,
publication, and accounting effects belong to the consuming device. Swapping an
executor changes only how the same stage graph is driven and never creates another
PDMA core, driver, or end-to-end path.

Consumers own request reset and persistent hardware lifecycle. Canceling a driving
future leaves unfinished operation state available for resumption. If the terminal
transition already committed, the operation remains exhausted and an output carried by
the canceled future is lost. Resetting an operation discards its request state without
replacing the connected hardware.
