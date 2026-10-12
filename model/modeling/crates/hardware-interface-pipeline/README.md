# Hardware interface pipeline

`hardware-interface-pipeline` supplies statically composed, deterministic hardware
value flow. It has no normal dependency on a device model, storage backend, locks,
or async runtime.

`Stage<C>` maps an owned `Input` to an owned `Output` using local invocation
`State`. `start(input)` performs no hardware effects. A bounded `step` returns
`StepResult<O,E> = Result<StepOutcome<O,E>, RunError>`. The only business outcomes
are `InProgress` and `Complete(Result<O,E>)`. Completion transfers the value and
leaves no unfinished work in that invocation. `RunError::Exhausted` is a driving
call error, never a stage result. There are no wait tokens or readiness interface.

`Operation<S,C>` owns invocation state independently of the hardware and its
driving future. Terminal transitions discard child state. Explicit reset discards
retained state and any uncollected terminal result, while committed external
effects remain. A canceled asynchronous driver can resume the same operation,
including an owned result retained across the mandatory terminal yield.

`Then<A,B>` transfers an upstream result to downstream state and returns
`InProgress`; downstream runs on a later step. `Repeat<S>` interprets a completed
`ControlFlow::Continue(input)` as a fresh child invocation, again without stepping
it in the same call. A child returning `InProgress` retains its exact state.
`AfterEach<A,B>` starts a finite `B: Stage<Input=(),Output=(),Error=Infallible>`
after every upstream step. If maintenance is incomplete, it retains the upstream
outcome and finishes maintenance before advancing upstream or publishing that
outcome. `MapError` converts application errors without converting call errors.

## Fan value flow

`Fan<S,P,T>::new(inputs, policy, outputs)` connects N input stages to M output
stages. One invocation accepts `Vec<S::Input>` matching the input population and
returns `Vec<(usize,T::Output)>` in output-completion order. Output sweeps use
ascending port order. Each source is invoked once per batch.

A batch alternates three bounded steps:

1. Advance every unfinished input at most once. Completed values enter distinct
   buffer slots; finished sources are not stepped again.
2. Ask `FanPolicy::plan` to observe the buffered `&[Option<Value>]` and idle-output
   bitmap. Validate its complete `FanPlan { inputs, outputs }` before consuming
   selected values, then call `route` with their ownership.
3. Advance every active output at most once, retaining incomplete child states
   and completed receipts. Output slots become reusable after completion.

An incomplete input never prevents peers from finishing or active outputs from
advancing. Unselected completed values remain buffered. Completion requires every
source to finish, every buffered value to be consumed, and every active output to
finish. A policy that permanently retains a value deliberately keeps its batch
in progress; synchronous callers can bound this with the immediate executor.

The policy receives values, never source stage objects or an execution context.
A source can emit a non-consuming readiness snapshot; an output accepting real
work must revalidate stale snapshots when its domain requires it. Policy-owned
round-robin position may persist with the hardware. Per-request payloads and
unfinished child states belong to the fan invocation.

Plan indices must exist and be unique within their axis. Selected inputs must
have completed values and selected outputs must be idle. A nonempty output plan
must consume at least one input; an empty output plan may explicitly discard
selected inputs. `route` must return exactly one owned input per selected output.
The framework never clones payloads. Application splitting, reduction, sharing,
or cloning is explicit in `route`. One value/state slot per input, one active
state per output, and at most N*M receipts bound buffering for a finite batch.
Plan or child failure clears invocation state without rolling back committed
effects, and stops later effects in that sweep.

`FanIn<S,P,T>::new(inputs, policy, output)` and
`FanOut<S,P,T>::new(input, policy, outputs)` are singleton-axis wrappers over
that same mechanism. Fan-in accepts a vector of source inputs; fan-out accepts
its single source input directly. Both return indexed receipt vectors.

`Arbitrate<A>` adapts `Arbitration<Value,Error>::select` into a singleton-output
value-selection policy. Unselected values remain until later selections.
`Dispatch<R>` adapts `Route<Value,Error>::target` into singleton-input routing.
Neither adapter executes child hardware or owns another engine. Application
reducers after receipt collection compose as ordinary downstream stages.

## Drivers

- `StepExecutor` advances one bounded step.
- `ImmediateExecutor` advances steps until completion or its nonzero budget,
  returning `RunError::BudgetExhausted` without discarding unfinished work.
- `AsyncExecutor` obtains closure-scoped context access from
  `AsyncInteractionBackend<C>::interact`, performs one bounded step, and yields
  cooperatively after every interaction, including incomplete and terminal steps.
  The backend's `yield_now` uses its scheduler's cooperative yield; Tokio adapters
  use `tokio::task::yield_now`. The backend has no hardware-wait protocol.

Context guards never escape synchronous interaction closures. Software interrupts,
device wakeup policy, memory ownership, lifecycle rules, and application scheduling
remain with their existing owners. Infinite hardware activity can compose finite
batches with `Then` and `Repeat`; fan itself is a finite invocation mechanism.
