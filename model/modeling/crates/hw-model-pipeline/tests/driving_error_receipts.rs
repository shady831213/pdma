//! Driving error receipts survive asynchronous cancellation without retiring hardware.
use hw_model_pipeline::{
    AsyncExecutor, AsyncInteractionBackend, AsyncRunError, HardwareDriver, Operation, RunError,
    Stage, StepOutcome, StepResult,
};
use std::{
    future::Future,
    sync::Mutex,
    task::{Context, Waker},
};

#[derive(Default)]
struct Backend(Mutex<Vec<&'static str>>);
impl AsyncInteractionBackend<Vec<&'static str>> for Backend {
    type Error = ();
    async fn yield_now(&self) {
        tokio::task::yield_now().await;
    }
    async fn interact<O: Send, E: Send, F>(&self, interaction: F) -> Result<StepResult<O, E>, ()>
    where
        F: FnOnce(&mut Vec<&'static str>) -> StepResult<O, E> + Send,
    {
        Ok(interaction(&mut *self.0.lock().map_err(|_| ())?))
    }
}

struct Recoverable {
    accepted: bool,
    terminal_error: bool,
    reject_offer: bool,
}
impl Stage<Vec<&'static str>> for Recoverable {
    type Input = ();
    type Output = ();
    type Error = ();
    fn step(
        &mut self,
        effects: &mut Vec<&'static str>,
        input: &mut Option<()>,
    ) -> StepResult<(), ()> {
        if std::mem::take(&mut self.reject_offer) {
            effects.push("rejected");
            return Err(RunError::BudgetExhausted);
        }
        if std::mem::take(&mut self.accepted) {
            effects.push("completed");
            return Ok(StepOutcome::Complete(if self.terminal_error {
                Err(())
            } else {
                Ok(())
            }));
        }
        if input.take().is_some() {
            self.accepted = true;
            effects.push("accepted");
            return Err(RunError::BudgetExhausted);
        }
        Ok(StepOutcome::InProgress)
    }
}

#[tokio::test]
async fn cancelled_operation_error_yield_retains_receipt_and_hardware_continuation() {
    // Given: hardware accepts a request and reports a driving error.
    let executor = AsyncExecutor::new(Backend::default());
    let mut hardware = Recoverable {
        accepted: false,
        terminal_error: false,
        reject_offer: false,
    };
    let mut operation = Operation::new(());
    // When: the error yield is cancelled before its receipt is delivered.
    let mut run = Box::pin(executor.run(&mut hardware, &mut operation));
    assert!(
        run.as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
    drop(run);
    assert!(!operation.is_complete());
    assert_eq!(
        executor.run(&mut hardware, &mut operation).await,
        Err(AsyncRunError::Run(RunError::BudgetExhausted))
    );
    assert_eq!(*executor.backend().0.lock().unwrap(), ["accepted"]);
    let result = executor.run(&mut hardware, &mut operation).await;
    // Then: the retained invocation completes exactly once after error delivery.
    assert_eq!(result, Ok(()));
    assert!(operation.is_complete());
    assert_eq!(
        *executor.backend().0.lock().unwrap(),
        ["accepted", "completed"]
    );
}

#[tokio::test]
async fn cancelled_driver_error_yield_retains_receipt_and_hardware_continuation() {
    // Given: a driver owns a service which can recover from a driving failure.
    let executor = AsyncExecutor::new(Backend::default());
    let mut hardware = Recoverable {
        accepted: false,
        terminal_error: true,
        reject_offer: false,
    };
    let mut driver = HardwareDriver::new();
    // When: cancellation occurs at the error yield, followed by two resumptions.
    let mut run = Box::pin(executor.drive(&mut hardware, &mut driver));
    assert!(
        run.as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
    drop(run);
    assert!(!driver.is_complete());
    assert_eq!(
        executor.drive(&mut hardware, &mut driver).await,
        Err(AsyncRunError::Run(RunError::BudgetExhausted))
    );
    assert_eq!(*executor.backend().0.lock().unwrap(), ["accepted"]);
    let result = executor.drive(&mut hardware, &mut driver).await;
    // Then: only the later domain completion terminates the hardware invocation.
    assert_eq!(result, Err(AsyncRunError::Stage(())));
    assert!(driver.is_complete());
    assert_eq!(
        *executor.backend().0.lock().unwrap(),
        ["accepted", "completed"]
    );
}

#[test]
fn driver_preserves_an_unaccepted_offer_after_raw_error() {
    // Given: driving fails before the hardware accepts its offered request.
    let mut hardware = Recoverable {
        accepted: false,
        terminal_error: true,
        reject_offer: true,
    };
    let mut driver = HardwareDriver::new();
    let mut effects = Vec::new();
    // When: driving is retried through rejection, acceptance and completion.
    assert_eq!(
        driver.step(&mut hardware, &mut effects),
        Err(RunError::BudgetExhausted)
    );
    assert_eq!(
        driver.step(&mut hardware, &mut effects),
        Err(RunError::BudgetExhausted)
    );
    let result = driver.step(&mut hardware, &mut effects);
    // Then: the original offer reaches hardware and completes once.
    assert_eq!(result, Ok(StepOutcome::Complete(Err(()))));
    assert_eq!(effects, ["rejected", "accepted", "completed"]);
}
