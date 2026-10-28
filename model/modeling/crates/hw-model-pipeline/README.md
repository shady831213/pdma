# hw-model-pipeline

`hw-model-pipeline` supplies statically composed, deterministic hardware
value flow. It has no normal dependency on a device model, storage backend, locks,
or async runtime.

`Stage<C>` maps an owned `Input` to an owned `Output`. Its transient processing
state lives inside the stage instance; there is no public State type or start method.
A bounded `step(context, &mut input_slot)` returns
`StepResult<O,E> = Result<StepOutcome<O,E>, RunError>`. The only business outcomes
are `InProgress` and `Complete(Result<O,E>)`. Completion transfers the value and
leaves no unfinished work for that accepted input. `RunError::Exhausted` is a driving
call error, never a stage result. There are no wait tokens or readiness interface.

The input slot is the ownership handshake. Taking its value accepts the request;
a busy stage leaves a new offer untouched. It can continue accepted work with an
empty slot, and does not accept a replacement on the step completing that work.
An idle stage with no offer returns `InProgress` without effects. For a stage whose
Input is `Option<T>`, outer `None` means no offer and `Some(None)` is an offered domain
bubble. Completion leaves the stage ready for a subsequent request.

The stage instance is the continuation. Only `step` advances it; the framework
exposes neither its private processing state nor an external cleanup hook.
Domain reset requests belong to context. A stage observes them while stepping,
releases its own unfinished work according to the domain protocol, and completes.
Combinators cannot force an unfinished child to become complete.

`Operation<S,C>` retains only the unaccepted input and terminal result, not a child
continuation. Accepted intermediate state stays in the hardware even when an
asynchronous driving future is canceled. Resuming uses the same hardware and
operation, including any result retained across the mandatory terminal yield.
A call error does not establish hardware completion: the operation and hardware
retain their continuation for subsequent driving. Dropping a driving future is
not a domain reset or a permission to reuse resources held by unfinished work.

`Then<A,B,V>` buffers the upstream result V for downstream and returns `InProgress`;
downstream runs on a later step. While downstream is active, the upstream input
slot is backpressured. A child returning `InProgress` retains its own state.
The edge type V is inferred by construction and checked against both ports.
`MapError` converts application errors without converting call errors.
Independent hardware units belong in the application's top-level assembly and
advance from the external tick. Their progress must not depend on how many
internal steps another pipeline takes. The framework has no per-child-step
maintenance wrapper or feedback combinator.

Stage invocation lifetime need not equal application task lifetime. A hardware
task may persist in explicitly owned context while successive finite passes
advance it. Application occupancy and readiness state supplies backpressure;
the top-level hardware assembly and external driver provide subsequent inputs.
No separate producer trait or command-lifetime streaming combinator is required.

## Fan value flow

`Fan::new(inputs, policy, outputs)` connects N input stages to M output
stages. One invocation accepts `Vec<S::Input>` matching the input population and
returns `Vec<(usize,T::Output)>` in output-completion order. Output sweeps use
ascending port order. Each source is invoked once per batch.

A batch rotates through three bounded steps, skipping phases with no work:

1. Advance every unfinished input at most once. Completed values enter distinct
   buffer slots; finished sources are not stepped again.
2. Ask `FanPolicy::plan` to observe the buffered `&[Option<Value>]` and idle-output
   bitmap. Validate its complete `FanPlan { inputs, outputs }` before consuming
   selected values, then call `route` with their ownership.
3. Advance every active output at most once, retaining input offers and completed
   receipts in the fan. Accepted progress remains in each child. Output slots become
   reusable after completion.

Planning runs only when completed values are buffered, including retained values
when all outputs are busy (a policy may discard them). Each tick performs at most
one meaningful phase. With only active outputs left, successive ticks advance
those outputs directly; running inputs and outputs retain their rotation order.

An incomplete input never prevents peers from finishing or active outputs from
advancing. Unselected completed values remain buffered. Completion requires every
source to finish, every buffered value to be consumed, and every active output to
finish. A policy that permanently retains a value deliberately keeps its batch
in progress; synchronous callers can bound this with the immediate executor.

The policy receives values, never source stage objects or an execution context.
A source can emit a non-consuming readiness snapshot; an output accepting real
work must revalidate stale snapshots when its domain requires it. Policy-owned
round-robin position may persist with the hardware. Buffered edge values belong
to the fan; unfinished child work belongs to its respective stage instance.

Plan indices must exist and be unique within their axis. Selected inputs must
have completed values and selected outputs must be idle. A nonempty output plan
must consume at least one input; an empty output plan may explicitly discard
selected inputs. `route` must return exactly one owned input per selected output.
The framework never clones payloads. Application splitting, reduction, sharing,
or cloning is explicit in `route`. One value/state slot per input, one active
state per output, and at most N*M receipts bound buffering for a finite batch.
On plan or child failure, the fan retains the first error, stops admitting new
work and discards only its own unaccepted offers and buffered values. Already
accepted children continue stepping until they complete. Only then does the fan
return the retained error. This neither cancels those children nor rolls back
their effects. A child call error preserves the batch and propagates to the driver;
it is not proof that the child has terminated.

`FanIn::new(inputs, policy, output)` and
`FanOut::new(input, policy, outputs)` are singleton-axis wrappers over
that same mechanism. Fan-in accepts a vector of source inputs; fan-out accepts
its single source input directly. Both return indexed receipt vectors.

Named fan types include their stored port value types:
`Fan<S,P,T,I,V,J,O,E>` (and the same parameters for FanIn/FanOut). I/V describe the
source input/output, J/O the destination input/output, and E the retained child
error while accepted peers finish. Constructor calls infer
them. Keeping only owned values in this storage preserves generic contexts and
freshly borrowed contexts without storing context lifetimes in the hardware.

`Arbitrate<A>` adapts `Arbitration<Value,Error>::select` into a singleton-output
value-selection policy. `Arbitrate::new` retains unselected owned values until
later selections. `Arbitrate::snapshots` waits for every input and an available
output, asks the same selector for one winner, and consumes the entire snapshot
round. Only the winner reaches the output; the remaining snapshots are discarded.
No winner completes the round without invoking the output. This constructor is
for observations that can be sampled again, not independently owned work that
must all execute. Both constructors use the same fan lifecycle and plan validation.
`Dispatch<R>` adapts `Route<Value,Error>::target` into singleton-input routing.
Neither adapter executes child hardware or owns another engine. Application
reducers after receipt collection compose as ordinary downstream stages.

## Drivers

- `StepExecutor` advances one bounded step.
- `HardwareDriver<S,C>` owns external driving state for a hardware service whose
  input and output are both unit. It retains the current input/result receipt while
  unfinished work stays in the stage. Successful completion returns `InProgress`; the next external
  step starts a fresh invocation. A stage error terminates the service. A raw call
  error returns to the driver caller while retaining unfinished work; another call
  resumes it without supplying a replacement input.
  It is not a `Stage` and cannot be inserted into a pipeline.
- `ImmediateExecutor` advances steps until completion or its nonzero budget,
  returning `RunError::BudgetExhausted` without discarding unfinished work.
- `AsyncExecutor` obtains closure-scoped context access from
  `AsyncInteractionBackend<C>::interact`, performs one bounded step, and yields
  cooperatively after every interaction, including incomplete and terminal steps.
  The backend's `yield_now` uses its scheduler's cooperative yield; Tokio adapters
  use `tokio::task::yield_now`. The backend has no hardware-wait protocol.
  `run` drives one retained operation; `drive` uses a caller-owned `HardwareDriver`
  to sustain finite hardware services. Cancellation preserves unfinished work,
  completed effects, and terminal errors awaiting delivery after a yield.

Context guards never escape synchronous interaction closures. Software interrupts,
device wakeup policy, memory ownership, lifecycle rules, and application scheduling
remain with their existing owners. Infinite hardware activity is driven externally
through finite services; fan itself is a finite invocation mechanism.

## Standalone example

Run `cargo run --example two_stage -p hw-model-pipeline` to compose a
byte-widening stage with a little-endian encoder. The first external step accepts
the value; the second delivers the encoded bytes. Both stages use only the public
pipeline API and the standard library.

Package names are provisional and publication remains disabled. Licensed under
MIT OR Apache-2.0, at your option; see LICENSE-MIT and LICENSE-APACHE.
