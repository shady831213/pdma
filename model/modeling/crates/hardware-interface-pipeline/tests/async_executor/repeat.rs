use std::{ops::ControlFlow, sync::atomic::Ordering};

use hardware_interface_pipeline::{AsyncExecutor, Operation, Repeat, Stage, StepOutcome};

use super::{
    Context,
    backend::{ScriptedBackend, lock},
    poll_once,
};

struct Countdown;

impl Stage<Context> for Countdown {
    type Input = u8;
    type Output = ControlFlow<u8, u8>;
    type Error = &'static str;
    type Wait = usize;
    type State = u8;

    fn start(input: Self::Input) -> Self::State {
        input
    }

    fn step(
        &mut self,
        context: &mut Context,
        state: Self::State,
    ) -> StepOutcome<Self::State, Self::Output, Self::Error, Self::Wait> {
        if state == 0 {
            context.push("finished");
            StepOutcome::Complete(Ok(ControlFlow::Break(7)))
        } else {
            context.push("continued");
            StepOutcome::Complete(Ok(ControlFlow::Continue(state - 1)))
        }
    }
}

#[tokio::test]
async fn cancellation_after_feedback_commit_resumes_the_next_iteration() {
    // Given: the backend suspends after committing a feedback interaction.
    let executor = AsyncExecutor::new(ScriptedBackend::configured(true, false, None));
    let mut hardware = Repeat::new(Countdown);
    let mut operation = Operation::new(1);

    // When: the driver is canceled after feedback and resumed with the backend open.
    let mut run = Box::pin(executor.run(&mut hardware, &mut operation));
    assert!(poll_once(run.as_mut()).is_pending());
    drop(run);
    executor.backend().after_open.store(true, Ordering::SeqCst);
    let result = executor.run(&mut hardware, &mut operation).await;

    // Then: the committed iteration is not replayed and the next iteration completes.
    assert_eq!(result, Ok(7));
    assert_eq!(
        *lock(&executor.backend().context),
        ["continued", "finished"]
    );
    assert_eq!(executor.backend().interactions.load(Ordering::SeqCst), 2);
}
