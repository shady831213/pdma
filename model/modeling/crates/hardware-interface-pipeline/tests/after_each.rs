#![expect(missing_docs, reason = "integration tests")]

use std::convert::Infallible;

use hardware_interface_pipeline::{AfterEach, Operation, Stage, StepOutcome};

struct Upstream;
struct Update;

impl Stage<Vec<u8>> for Upstream {
    type Input = StepOutcome<u8, u8, u8>;
    type Output = u8;
    type Error = u8;
    type Wait = u8;
    type State = Option<Self::Input>;

    fn start(input: Self::Input) -> Self::State {
        Some(input)
    }

    fn step(&mut self, context: &mut Vec<u8>, state: &mut Self::State) -> Self::Input {
        let Some(outcome) = state.take() else {
            return StepOutcome::Exhausted;
        };
        context.push(1);
        outcome
    }
}

impl Stage<Vec<u8>> for Update {
    type Input = ();
    type Output = Infallible;
    type Error = Infallible;
    type Wait = Infallible;
    type State = ();

    fn start((): Self::Input) -> Self::State {}

    fn step(
        &mut self,
        context: &mut Vec<u8>,
        (): &mut Self::State,
    ) -> StepOutcome<Self::Output, Self::Error, Self::Wait> {
        context.push(2);
        StepOutcome::Progressed
    }
}

#[test]
fn update_runs_after_progress_pending_success_and_error_without_replacing_the_outcome() {
    // Given: a command stage followed by an always-ready hardware update.
    for (outcome, expected) in [
        (StepOutcome::Progressed, StepOutcome::Progressed),
        (StepOutcome::Pending(7), StepOutcome::Pending(7)),
        (StepOutcome::Complete(Ok(11)), StepOutcome::Complete(Ok(11))),
        (
            StepOutcome::Complete(Err(13)),
            StepOutcome::Complete(Err(13)),
        ),
    ] {
        let mut hardware = AfterEach::new(Upstream, Update);
        let mut operation = Operation::new(outcome);
        let mut context = Vec::new();

        // When: the composed hardware performs a single step.
        let result = operation.step(&mut hardware, &mut context);

        // Then: the update follows the upstream effect, preserving its exact result.
        assert_eq!(context, [1, 2]);
        assert_eq!(result, expected);
        assert_eq!(
            operation.step(&mut hardware, &mut context),
            StepOutcome::Exhausted
        );
        assert_eq!(context, [1, 2]);
    }
}
