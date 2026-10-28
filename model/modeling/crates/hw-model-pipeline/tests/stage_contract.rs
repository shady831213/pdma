//! Consumer-visible contract tests for explicitly stepped stages.

use hw_model_pipeline::{Operation, RunError, Stage, StepOutcome, StepResult};

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
struct Request {
    state: State,
    completion: Option<Result<OwnedOutput, OwnedError>>,
}

#[derive(Default)]
struct ContractStage(Option<Request>);

impl Stage<Context> for ContractStage {
    type Input = Result<OwnedOutput, OwnedError>;
    type Error = OwnedError;
    type Output = OwnedOutput;

    fn step(
        &mut self,
        context: &mut Context,
        input: &mut Option<Self::Input>,
    ) -> StepResult<Self::Output, Self::Error> {
        if self.0.is_none() {
            let Some(input) = input.take() else {
                return Ok(StepOutcome::InProgress);
            };
            self.0 = Some(Request {
                state: State::Start,
                completion: Some(input),
            });
        }
        let mut request = self.0.take().ok_or(RunError::Exhausted)?;
        match request.state {
            State::Start => {
                context.interactions += 1;
                request.state = State::Waiting;
                self.0 = Some(request);
                Ok(StepOutcome::InProgress)
            }
            State::Waiting => {
                request.state = State::Ready;
                self.0 = Some(request);
                Ok(StepOutcome::InProgress)
            }
            State::Ready => {
                request.state = State::Finished;
                request
                    .completion
                    .take()
                    .map_or(Err(RunError::Exhausted), |value| {
                        Ok(StepOutcome::Complete(value))
                    })
            }
            State::Finished => Err(RunError::Exhausted),
        }
    }
}

#[test]
fn all_outcomes_transfer_owned_values_and_terminal_stage_is_exhausted() {
    // Given: one reusable stage and requests owning non-Copy success and error values.
    let mut hardware = ContractStage::default();
    let mut success = Operation::new(Ok(OwnedOutput("owned output".to_owned())));
    let mut failure = Operation::new(Err(OwnedError("owned error".to_owned())));
    let mut success_context = Context::default();
    let mut failure_context = Context::default();

    // When: consumers drive each stage across its visible boundaries.
    let started = success.step(&mut hardware, &mut success_context);
    let unfinished = success.step(&mut hardware, &mut success_context);
    let complete_ok = success.step(&mut hardware, &mut success_context);
    let complete_err = {
        let _ = failure.step(&mut hardware, &mut failure_context);
        let _ = failure.step(&mut hardware, &mut failure_context);
        failure.step(&mut hardware, &mut failure_context)
    };
    let exhausted = failure.step(&mut hardware, &mut failure_context);

    // Then: every literal outcome transfers its owned value and terminal state stays exhausted.
    assert_eq!(started, Ok(StepOutcome::InProgress));
    println!("InProgress after interaction");
    assert_eq!(unfinished, Ok(StepOutcome::InProgress));
    println!("InProgress with retained invocation");
    assert_eq!(
        complete_ok,
        Ok(StepOutcome::Complete(Ok(OwnedOutput(
            "owned output".to_owned()
        ))))
    );
    println!("Complete(Ok(owned non-Copy output))");
    assert_eq!(
        complete_err,
        Ok(StepOutcome::Complete(Err(OwnedError(
            "owned error".to_owned()
        ))))
    );
    println!("Complete(Err(owned non-Copy error))");
    assert_eq!(exhausted, Err(RunError::Exhausted));
    println!("Exhausted(post-terminal)");
    assert_eq!(
        success.step(&mut hardware, &mut success_context),
        Err(RunError::Exhausted)
    );
}

#[test]
fn unfinished_step_can_be_discarded_and_stage_resumes_without_repeating_interaction() {
    // Given: a stage advanced through one interaction with retained work.
    let mut hardware = ContractStage::default();
    let mut operation = Operation::new(Ok(OwnedOutput("owned output".to_owned())));
    let mut context = Context::default();
    assert_eq!(
        operation.step(&mut hardware, &mut context),
        Ok(StepOutcome::InProgress)
    );
    let pending = operation.step(&mut hardware, &mut context);

    // When: the consumer drops the unfinished outcome and later resumes the stage.
    drop(pending);
    let resumed = operation.step(&mut hardware, &mut context);

    // Then: the committed interaction is not repeated and completion remains owned.
    assert_eq!(context.interactions, 1);
    assert_eq!(
        resumed,
        Ok(StepOutcome::Complete(Ok(OwnedOutput(
            "owned output".to_owned()
        ))))
    );
    assert_eq!(
        operation.step(&mut hardware, &mut context),
        Err(RunError::Exhausted)
    );
}
