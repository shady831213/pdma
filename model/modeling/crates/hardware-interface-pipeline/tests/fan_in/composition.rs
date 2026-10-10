use super::policy::Policy;
use std::ops::ControlFlow;

use hardware_interface_pipeline::{
    Arbitrate, Operation, Repeat, RunOutcome, Stage, StepOutcome, Then,
};

use super::sources::{Context, Job, Queue};

struct Collect {
    values: Vec<u8>,
}

impl Stage<Context> for Collect {
    type Input = u8;
    type Output = ControlFlow<Vec<u8>>;
    type Error = hardware_interface_pipeline::BranchError<&'static str>;
    type Wait = &'static str;
    type State = u8;

    fn start(input: u8) -> Self::State {
        input
    }

    fn step(
        &mut self,
        _: &mut Context,
        state: Self::State,
    ) -> StepOutcome<Self::State, Self::Output, Self::Error, Self::Wait> {
        let value = state;
        self.values.push(value);
        StepOutcome::Complete(Ok(if self.values.len() == 4 {
            ControlFlow::Break(std::mem::take(&mut self.values))
        } else {
            ControlFlow::Continue(())
        }))
    }
}

#[test]
fn then_and_repeat_preserve_round_robin_across_iteration_state() {
    // Given: two queues feeding a collector through the existing compositors.
    let mut context = Context::default();
    context.enqueue(0, [Job::Value(10), Job::Value(11)]);
    context.enqueue(1, [Job::Value(20), Job::Value(21)]);
    let fan_in = Arbitrate::new(
        vec![Queue::new(0), Queue::new(1)],
        Policy::round_robin("idle"),
    );
    let mut hardware = Repeat::new(Then::new(fan_in, Collect { values: Vec::new() }));
    let mut operation = Operation::new(());

    // When: four complete iterations are stepped through fan-in and downstream stages.
    let outcomes: Vec<_> = (0..16)
        .map(|_| operation.step(&mut hardware, &mut context))
        .collect();

    // Then: iteration recreation preserves fairness and forwards one terminal collection.
    assert!(
        outcomes
            .iter()
            .take(15)
            .all(|outcome| *outcome == Ok(RunOutcome::Progressed))
    );
    assert_eq!(
        outcomes.last(),
        Some(&Ok(RunOutcome::Complete(Ok(vec![10, 20, 11, 21]))))
    );
    assert_eq!(context.consumed, [0, 1, 0, 1]);
}
