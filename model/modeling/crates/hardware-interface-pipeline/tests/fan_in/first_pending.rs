use hardware_interface_pipeline::{Arbitrate, Operation, ReadyStage, Stage, StepOutcome};

use super::{
    policy::Policy,
    sources::{Context, Job, Queue, State},
};

struct PendingQueue(Queue);

impl Stage<Context> for PendingQueue {
    type Input = ();
    type Output = u8;
    type Error = &'static str;
    type Wait = &'static str;
    type State = State;

    fn start((): ()) -> State {
        Queue::start(())
    }

    fn step(
        &mut self,
        context: &mut Context,
        state: &mut State,
    ) -> StepOutcome<u8, Self::Error, Self::Wait> {
        match self.0.step(context, state) {
            StepOutcome::Progressed => StepOutcome::Pending("source release"),
            outcome => outcome,
        }
    }
}

impl ReadyStage<Context> for PendingQueue {
    fn ready(&self, context: &Context) -> bool {
        self.0.ready(context)
    }
}

#[test]
fn first_pending_child_retains_work_after_readiness_disappears() {
    // Given: the selected source accepts a value on its first Pending step.
    let mut context = Context::default();
    context.enqueue(0, [Job::Value(8)]);
    context.enqueue(1, [Job::Value(9)]);
    let mut hardware = Arbitrate::new(
        vec![
            PendingQueue(Queue {
                id: 0,
                delayed: true,
            }),
            PendingQueue(Queue::new(1)),
        ],
        Policy::round_robin("idle"),
    );
    let mut operation = Operation::new(());
    assert_eq!(
        operation.step(&mut hardware, &mut context),
        StepOutcome::Progressed
    );

    // When: the first child call suspends, then completion resumes after release.
    let accepted = operation.step(&mut hardware, &mut context);
    let pending = operation.step(&mut hardware, &mut context);
    context.released = true;
    let completed_child = operation.step(&mut hardware, &mut context);
    let result = operation.step(&mut hardware, &mut context);

    // Then: the empty source is never rechecked and its accepted output is retained.
    assert_eq!(accepted, StepOutcome::Pending("source release"));
    assert_eq!(pending, accepted);
    assert_eq!(completed_child, StepOutcome::Progressed);
    assert_eq!(result, StepOutcome::Complete(Ok(8)));
    assert_eq!(*context.probes.borrow(), [0, 0]);
    assert_eq!(context.consumed, [0]);
}
