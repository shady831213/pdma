use std::{future::Future, sync::atomic::Ordering, task::Poll};

use hardware_interface_pipeline::{
    AsyncExecutor, AsyncRunError, HardwareDriver, RunError, Stage, StepOutcome, StepResult,
};

use super::{
    Context,
    backend::{ScriptedBackend, lock},
    poll_once,
};

async fn finish<T>(future: impl Future<Output = T>) -> Option<T> {
    let mut future = Box::pin(future);
    for _ in 0..32 {
        match poll_once(future.as_mut()) {
            Poll::Ready(result) => return Some(result),
            Poll::Pending => tokio::task::yield_now().await,
        }
    }
    None
}

struct Service {
    completed: bool,
    failure: bool,
    accepted: bool,
}

impl Stage<Context> for Service {
    type Input = ();
    type Output = ();
    type Error = &'static str;

    fn step(
        &mut self,
        context: &mut Context,
        input: &mut Option<()>,
    ) -> StepResult<(), Self::Error> {
        if self.completed {
            let Some(()) = input.take() else {
                return Ok(StepOutcome::InProgress);
            };
            context.push("stopped");
            return Ok(StepOutcome::Complete(Err("stop")));
        }
        if !self.accepted {
            let Some(()) = input.take() else {
                return Ok(StepOutcome::InProgress);
            };
            self.accepted = true;
            context.push("accepted");
            return Ok(StepOutcome::InProgress);
        }
        if self.failure {
            self.failure = false;
            context.push("interrupted");
            Err(RunError::BudgetExhausted)
        } else {
            self.completed = true;
            self.accepted = false;
            context.push("committed");
            Ok(StepOutcome::Complete(Ok(())))
        }
    }
}

#[tokio::test]
async fn cancellation_after_acceptance_preserves_the_active_invocation() {
    // Given: the backend suspends after the first committed interaction.
    let executor = AsyncExecutor::new(ScriptedBackend::configured(true, false, None));
    let mut hardware = Service {
        completed: false,
        failure: false,
        accepted: false,
    };
    let mut driver = HardwareDriver::new();

    // When: driving is canceled after acceptance and resumed with the backend open.
    let mut run = Box::pin(executor.drive(&mut hardware, &mut driver));
    assert!(poll_once(run.as_mut()).is_pending());
    drop(run);
    executor.backend().after_open.store(true, Ordering::SeqCst);
    let result = finish(executor.drive(&mut hardware, &mut driver)).await;

    // Then: the retained invocation commits once before the next invocation stops.
    assert_eq!(result, Some(Err(AsyncRunError::Stage("stop"))));
    assert_eq!(
        *lock(&executor.backend().context),
        ["accepted", "committed", "stopped"]
    );
    assert!(driver.is_complete());
}

#[tokio::test]
async fn cancellation_at_successful_completion_yield_does_not_replay_effects() {
    // Given: one accepted invocation is about to complete successfully.
    let executor = AsyncExecutor::new(ScriptedBackend::configured(true, true, None));
    let mut hardware = Service {
        completed: false,
        failure: false,
        accepted: false,
    };
    let mut driver = HardwareDriver::new();
    assert_eq!(
        driver.step(&mut hardware, &mut lock(&executor.backend().context)),
        Ok(StepOutcome::InProgress)
    );

    // When: the successful completion yield is canceled before driving resumes.
    let mut run = Box::pin(executor.drive(&mut hardware, &mut driver));
    assert!(poll_once(run.as_mut()).is_pending());
    drop(run);
    assert_eq!(
        *lock(&executor.backend().context),
        ["accepted", "committed"]
    );
    assert!(!driver.is_complete());
    let result = finish(executor.drive(&mut hardware, &mut driver)).await;

    // Then: a new invocation starts without replaying the completed invocation.
    assert_eq!(result, Some(Err(AsyncRunError::Stage("stop"))));
    assert_eq!(
        *lock(&executor.backend().context),
        ["accepted", "committed", "stopped"]
    );
    assert_eq!(executor.backend().interactions.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn cancellation_at_failure_yield_retains_typed_and_raw_failure_receipts() {
    // Given: either a stage failure or a raw run failure occurs on the next step.
    for raw in [false, true] {
        let executor = AsyncExecutor::new(ScriptedBackend::configured(true, true, None));
        let mut hardware = Service {
            completed: !raw,
            failure: raw,
            accepted: false,
        };
        let mut driver = HardwareDriver::new();
        if raw {
            assert_eq!(
                driver.step(&mut hardware, &mut Vec::new()),
                Ok(StepOutcome::InProgress)
            );
        }

        // When: delivery is canceled at the failure yield and retried.
        let mut run = Box::pin(executor.drive(&mut hardware, &mut driver));
        assert!(poll_once(run.as_mut()).is_pending());
        drop(run);
        assert_eq!(driver.is_complete(), !raw);
        let result = finish(executor.drive(&mut hardware, &mut driver)).await;
        let retried = finish(executor.drive(&mut hardware, &mut driver)).await;

        // Then: the original receipt survives and raw failure resumes the accepted invocation.
        let expected = if raw {
            AsyncRunError::Run(RunError::BudgetExhausted)
        } else {
            AsyncRunError::Stage("stop")
        };
        assert_eq!(result, Some(Err(expected)));
        if raw {
            assert_eq!(retried, Some(Err(AsyncRunError::Stage("stop"))));
            assert_eq!(
                *lock(&executor.backend().context),
                ["interrupted", "committed", "stopped"]
            );
        } else {
            assert_eq!(retried, Some(Err(AsyncRunError::Run(RunError::Exhausted))));
            assert_eq!(*lock(&executor.backend().context), ["stopped"]);
        }
    }
}
