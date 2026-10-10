#![expect(missing_docs, reason = "integration tests")]

use std::convert::Infallible;

use hardware_interface_pipeline::{AfterEach, Operation, RunOutcome, Stage, StepOutcome};

struct Upstream;
struct Update;

impl Stage<Vec<u8>> for Upstream {
    type Input = RunOutcome<u8, u8, u8>;
    type Output = u8;
    type Error = u8;
    type Wait = u8;
    type State = Self::Input;

    fn start(input: Self::Input) -> Self::State {
        input
    }

    fn step(
        &mut self,
        context: &mut Vec<u8>,
        state: Self::State,
    ) -> StepOutcome<Self::State, u8, u8, u8> {
        context.push(1);
        match state {
            RunOutcome::Progressed => StepOutcome::Progressed(RunOutcome::Progressed),
            RunOutcome::Pending(wait) => StepOutcome::Pending(RunOutcome::Pending(wait), wait),
            RunOutcome::Complete(result) => StepOutcome::Complete(result),
        }
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
        (): Self::State,
    ) -> StepOutcome<Self::State, Self::Output, Self::Error, Self::Wait> {
        context.push(2);
        StepOutcome::Progressed(())
    }
}

#[test]
fn update_runs_after_progress_pending_success_and_error_without_replacing_the_outcome() {
    // Given: a command stage followed by an always-ready hardware update.
    for (outcome, expected) in [
        (RunOutcome::Progressed, Ok(RunOutcome::Progressed)),
        (RunOutcome::Pending(7), Ok(RunOutcome::Pending(7))),
        (
            RunOutcome::Complete(Ok(11)),
            Ok(RunOutcome::Complete(Ok(11))),
        ),
        (
            RunOutcome::Complete(Err(13)),
            Ok(RunOutcome::Complete(Err(13))),
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
        if matches!(result, Ok(RunOutcome::Progressed | RunOutcome::Pending(_))) {
            operation.reset();
        }
        assert_eq!(
            operation.step(&mut hardware, &mut context),
            Err(hardware_interface_pipeline::AlreadyComplete)
        );
        assert_eq!(context, [1, 2]);
    }
}
