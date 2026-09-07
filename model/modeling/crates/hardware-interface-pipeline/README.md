# Hardware interface pipeline

`hardware-interface-pipeline` is a workspace-private, runtime-neutral algebra for
statically composed hardware interactions. It depends on no storage runtime, lock,
async runtime, or device model.

`Stage<C>` performs at most one architecture-visible interaction per `step`. A stage
advances its private state before reporting `Progressed`. `StepOutcome` transfers owned
wait, output, and error values as `Progressed`, `Pending`, `Complete`, or `Exhausted`.
Polling a completed stage is therefore explicit and non-panicking.

`Then` stores a completed upstream output and constructs its downstream stage exactly
once. The construction step returns `Progressed` without polling downstream, and a
downstream `Pending` resumes downstream without replaying upstream work.

The executors select interaction drive policy, not device behavior:

- `StepExecutor` advances exactly one leaf step.
- `ImmediateExecutor` loops only across `Progressed` outcomes and returns an owned wait
  token, terminal result, or exhaustion error.
- `AsyncExecutor<B>` asks an `AsyncInteractionBackend` for one closure-scoped context
  interaction per leaf step, then awaits an owned wait token after the closure ends.

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

PDMA's command path composes fetch, operation, and retirement with `Then`. Its async
worker runs through `AsyncExecutor` using a borrowed device backend; command state
remains owned by the device core so reset can discard retained work synchronously.
