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

The `src/compositor/` module groups `Then`, `AfterEach`, and `Repeat` in their own
`then.rs`, `after_each.rs`, and `repeat.rs` files. Their types remain re-exported
from the crate root; execution policies live separately under `src/executor/`.

`Then<A, B>` owns both hardware stages and requires `A::Output = B::Input`, with matching
error and wait types. `upstream.then(downstream)` connects existing hardware. Each
operation transfers an upstream result into downstream state exactly once and returns
`Progressed` without stepping downstream. A downstream `Pending` resumes that retained
state without replaying upstream work. Public `upstream` and `downstream` fields keep
hardware configuration and counters accessible between operations.

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
