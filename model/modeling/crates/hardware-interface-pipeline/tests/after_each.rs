#![expect(missing_docs, reason = "integration tests")]

use std::convert::Infallible;

use hardware_interface_pipeline::{AfterEach, Operation, RunError, Stage, StepOutcome, StepResult};

struct Upstream;
struct Update;

impl Stage<Vec<u8>> for Upstream {
    type Input = StepResult<u8, u8>;
    type Output = u8;
    type Error = u8;
    type State = Option<Self::Input>;

    fn start(input: Self::Input) -> Self::State {
        Some(input)
    }

    fn step(&mut self, context: &mut Vec<u8>, state: &mut Self::State) -> Self::Input {
        let Some(outcome) = state.take() else {
            return Err(RunError::Exhausted);
        };
        context.push(1);
        outcome
    }
}

impl Stage<Vec<u8>> for Update {
    type Input = ();
    type Output = ();
    type Error = Infallible;
    type State = ();

    fn start((): Self::Input) -> Self::State {}

    fn step(
        &mut self,
        context: &mut Vec<u8>,
        (): &mut Self::State,
    ) -> StepResult<Self::Output, Self::Error> {
        context.push(2);
        Ok(StepOutcome::Complete(Ok(())))
    }
}

#[test]
fn update_runs_after_unfinished_success_and_error_without_replacing_the_outcome() {
    // Given: a command stage followed by an always-ready hardware update.
    for (outcome, expected) in [
        (Ok(StepOutcome::InProgress), Ok(StepOutcome::InProgress)),
        (
            Ok(StepOutcome::Complete(Ok(11))),
            Ok(StepOutcome::Complete(Ok(11))),
        ),
        (
            Ok(StepOutcome::Complete(Err(13))),
            Ok(StepOutcome::Complete(Err(13))),
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
            Err(RunError::Exhausted)
        );
        assert_eq!(context, [1, 2]);
    }
}

struct TwoStepUpdate;

impl Stage<Vec<u8>> for TwoStepUpdate {
    type Input = ();
    type Output = ();
    type Error = Infallible;
    type State = bool;

    fn start((): ()) -> bool {
        false
    }

    fn step(&mut self, context: &mut Vec<u8>, started: &mut bool) -> StepResult<(), Infallible> {
        if *started {
            context.push(3);
            Ok(StepOutcome::Complete(Ok(())))
        } else {
            *started = true;
            context.push(2);
            Ok(StepOutcome::InProgress)
        }
    }
}

#[test]
fn upstream_result_is_retained_until_finite_update_completes() {
    for result in [None, Some(Ok(11)), Some(Err(13))] {
        // Given: an upstream outcome followed by a two-step maintenance operation.
        let mut hardware = AfterEach::new(Upstream, TwoStepUpdate);
        let mut operation = Operation::new(Ok(
            result.map_or(StepOutcome::InProgress, StepOutcome::Complete)
        ));
        let mut context = Vec::new();
        // When: the maintenance operation suspends and later completes.
        let unfinished = operation.step(&mut hardware, &mut context);
        assert_eq!(unfinished, Ok(StepOutcome::InProgress));
        assert_eq!(context, [1, 2]);
        let completed = operation.step(&mut hardware, &mut context);
        // Then: the upstream result arrives only after maintenance, without upstream replay.
        assert_eq!(
            completed,
            Ok(result.map_or(StepOutcome::InProgress, StepOutcome::Complete))
        );
        assert_eq!(context, [1, 2, 3]);
        assert_eq!(
            operation.step(&mut hardware, &mut context),
            Err(RunError::Exhausted)
        );
        assert_eq!(context, [1, 2, 3]);
    }
}
