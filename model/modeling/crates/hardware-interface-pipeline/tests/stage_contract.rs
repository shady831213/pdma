//! Consumer-visible contract tests for explicitly stepped stages.

use hardware_interface_pipeline::{Stage, StepOutcome};

#[derive(Debug, PartialEq, Eq)]
struct OwnedWait(String);

#[derive(Debug, PartialEq, Eq)]
struct OwnedOutput(String);

#[derive(Debug, PartialEq, Eq)]
struct OwnedError(String);

#[derive(Debug, Default)]
struct Context {
    interactions: usize,
}

#[derive(Debug)]
enum State {
    Start,
    Waiting,
    Ready,
    Finished,
}

#[derive(Debug)]
struct ContractStage {
    state: State,
    completion: Result<OwnedOutput, OwnedError>,
}

impl ContractStage {
    fn succeeding() -> Self {
        Self {
            state: State::Start,
            completion: Ok(OwnedOutput("owned output".to_owned())),
        }
    }

    fn failing() -> Self {
        Self {
            state: State::Start,
            completion: Err(OwnedError("owned error".to_owned())),
        }
    }
}

impl Stage<Context> for ContractStage {
    type Error = OwnedError;
    type Output = OwnedOutput;
    type Wait = OwnedWait;

    fn step(
        &mut self,
        context: &mut Context,
    ) -> StepOutcome<Self::Output, Self::Error, Self::Wait> {
        match self.state {
            State::Start => {
                context.interactions += 1;
                self.state = State::Waiting;
                StepOutcome::Progressed
            }
            State::Waiting => {
                self.state = State::Ready;
                StepOutcome::Pending(OwnedWait("owned wait".to_owned()))
            }
            State::Ready => {
                self.state = State::Finished;
                StepOutcome::Complete(std::mem::replace(
                    &mut self.completion,
                    Err(OwnedError("unreachable replacement".to_owned())),
                ))
            }
            State::Finished => StepOutcome::Exhausted,
        }
    }
}

#[test]
fn all_outcomes_transfer_owned_values_and_terminal_stage_is_exhausted() {
    // Given: independent stages that own non-Copy success, error, and wait values.
    let mut success = ContractStage::succeeding();
    let mut failure = ContractStage::failing();
    let mut success_context = Context::default();
    let mut failure_context = Context::default();

    // When: consumers drive each stage across its visible boundaries.
    let progressed = success.step(&mut success_context);
    let pending = success.step(&mut success_context);
    let complete_ok = success.step(&mut success_context);
    let complete_err = {
        let _ = failure.step(&mut failure_context);
        let _ = failure.step(&mut failure_context);
        failure.step(&mut failure_context)
    };
    let exhausted = failure.step(&mut failure_context);

    // Then: every literal outcome transfers its owned value and terminal state stays exhausted.
    assert_eq!(progressed, StepOutcome::Progressed);
    println!("Progressed");
    assert_eq!(
        pending,
        StepOutcome::Pending(OwnedWait("owned wait".to_owned()))
    );
    println!("Pending(owned non-Copy wait token)");
    assert_eq!(
        complete_ok,
        StepOutcome::Complete(Ok(OwnedOutput("owned output".to_owned())))
    );
    println!("Complete(Ok(owned non-Copy output))");
    assert_eq!(
        complete_err,
        StepOutcome::Complete(Err(OwnedError("owned error".to_owned())))
    );
    println!("Complete(Err(owned non-Copy error))");
    assert_eq!(exhausted, StepOutcome::Exhausted);
    println!("Exhausted(post-terminal)");
    assert_eq!(success.step(&mut success_context), StepOutcome::Exhausted);
}

#[test]
fn pending_wait_can_be_cancelled_and_stage_resumes_without_repeating_interaction() {
    // Given: a stage advanced through one interaction to an owned wait boundary.
    let mut stage = ContractStage::succeeding();
    let mut context = Context::default();
    assert_eq!(stage.step(&mut context), StepOutcome::Progressed);
    let pending = stage.step(&mut context);

    // When: the consumer cancels by dropping the owned wait and later resumes the stage.
    drop(pending);
    let resumed = stage.step(&mut context);

    // Then: the committed interaction is not repeated and completion remains owned.
    assert_eq!(context.interactions, 1);
    assert_eq!(
        resumed,
        StepOutcome::Complete(Ok(OwnedOutput("owned output".to_owned())))
    );
    assert_eq!(stage.step(&mut context), StepOutcome::Exhausted);
}
