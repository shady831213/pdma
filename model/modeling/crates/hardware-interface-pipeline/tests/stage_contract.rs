//! Consumer-visible contract tests for explicitly stepped stages.

use hardware_interface_pipeline::{Operation, RunOutcome, Stage, StepOutcome};

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
}

#[derive(Debug)]
struct Request {
    state: State,
    completion: Result<OwnedOutput, OwnedError>,
}

struct ContractStage;

impl Stage<Context> for ContractStage {
    type Input = Result<OwnedOutput, OwnedError>;
    type Error = OwnedError;
    type Output = OwnedOutput;
    type Wait = OwnedWait;
    type State = Request;

    fn start(input: Self::Input) -> Self::State {
        Request {
            state: State::Start,
            completion: input,
        }
    }

    fn step(
        &mut self,
        context: &mut Context,
        mut request: Self::State,
    ) -> StepOutcome<Self::State, Self::Output, Self::Error, Self::Wait> {
        match request.state {
            State::Start => {
                context.interactions += 1;
                request.state = State::Waiting;
                StepOutcome::Progressed(request)
            }
            State::Waiting => {
                request.state = State::Ready;
                StepOutcome::Pending(request, OwnedWait("owned wait".to_owned()))
            }
            State::Ready => StepOutcome::Complete(request.completion),
        }
    }
}

#[test]
fn all_outcomes_transfer_owned_values_and_completed_operation_is_inert() {
    // Given: one reusable stage and requests owning non-Copy success, error, and wait values.
    let mut hardware = ContractStage;
    let mut success = Operation::new(Ok(OwnedOutput("owned output".to_owned())));
    let mut failure = Operation::new(Err(OwnedError("owned error".to_owned())));
    let mut success_context = Context::default();
    let mut failure_context = Context::default();

    // When: consumers drive each stage across its visible boundaries.
    let progressed = success.step(&mut hardware, &mut success_context);
    let pending = success.step(&mut hardware, &mut success_context);
    let complete_ok = success.step(&mut hardware, &mut success_context);
    let complete_err = {
        let _ = failure.step(&mut hardware, &mut failure_context);
        let _ = failure.step(&mut hardware, &mut failure_context);
        failure.step(&mut hardware, &mut failure_context)
    };
    let exhausted = failure.step(&mut hardware, &mut failure_context);

    // Then: every literal outcome transfers its owned value and the completed operation stays inert.
    assert_eq!(progressed, Ok(RunOutcome::Progressed));
    println!("Progressed");
    assert_eq!(
        pending,
        Ok(RunOutcome::Pending(OwnedWait("owned wait".to_owned())))
    );
    println!("Pending(owned non-Copy wait token)");
    assert_eq!(
        complete_ok,
        Ok(RunOutcome::Complete(Ok(OwnedOutput(
            "owned output".to_owned()
        ))))
    );
    println!("Complete(Ok(owned non-Copy output))");
    assert_eq!(
        complete_err,
        Ok(RunOutcome::Complete(Err(OwnedError(
            "owned error".to_owned()
        ))))
    );
    println!("Complete(Err(owned non-Copy error))");
    assert_eq!(exhausted, Err(hardware_interface_pipeline::AlreadyComplete));
    println!("AlreadyComplete(post-terminal)");
    assert_eq!(
        success.step(&mut hardware, &mut success_context),
        Err(hardware_interface_pipeline::AlreadyComplete)
    );
}

#[test]
fn pending_wait_can_be_cancelled_and_stage_resumes_without_repeating_interaction() {
    // Given: a stage advanced through one interaction to an owned wait boundary.
    let mut hardware = ContractStage;
    let mut operation = Operation::new(Ok(OwnedOutput("owned output".to_owned())));
    let mut context = Context::default();
    assert_eq!(
        operation.step(&mut hardware, &mut context),
        Ok(RunOutcome::Progressed)
    );
    let pending = operation.step(&mut hardware, &mut context);

    // When: the consumer cancels by dropping the owned wait and later resumes the stage.
    drop(pending);
    let resumed = operation.step(&mut hardware, &mut context);

    // Then: the committed interaction is not repeated and completion remains owned.
    assert_eq!(context.interactions, 1);
    assert_eq!(
        resumed,
        Ok(RunOutcome::Complete(Ok(OwnedOutput(
            "owned output".to_owned()
        ))))
    );
    assert_eq!(
        operation.step(&mut hardware, &mut context),
        Err(hardware_interface_pipeline::AlreadyComplete)
    );
}
