//! Cooperative scheduling at the asynchronous driver boundary.

use hw_model_pipeline::{
    AsyncExecutor, AsyncInteractionBackend, AsyncRunError, Operation, RunError, Stage, StepOutcome,
    StepResult,
};
use std::{
    future::Future,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    task::{Context, Waker},
};

struct ReadyBackend(Mutex<usize>);

impl AsyncInteractionBackend<usize> for ReadyBackend {
    type Error = ();

    async fn yield_now(&self) {
        tokio::task::yield_now().await;
    }

    async fn interact<O: Send, E: Send, F>(&self, interaction: F) -> Result<StepResult<O, E>, ()>
    where
        F: FnOnce(&mut usize) -> StepResult<O, E> + Send,
    {
        Ok(interaction(&mut *self.0.lock().map_err(|_| ())?))
    }
}

#[derive(Default)]
struct BoundedWork(bool);

impl Stage<usize> for BoundedWork {
    type Input = ();
    type Output = ();
    type Error = ();
    fn step(&mut self, count: &mut usize, input: &mut Option<()>) -> StepResult<(), ()> {
        if !self.0 {
            let Some(()) = input.take() else {
                return Ok(StepOutcome::InProgress);
            };
            self.0 = true;
        }
        *count += 1;
        if *count < 4 {
            Ok(StepOutcome::InProgress)
        } else {
            self.0 = false;
            Ok(StepOutcome::Complete(Ok(())))
        }
    }
}

#[test]
fn ready_backend_yields_before_driving_another_step() {
    // Given: an immediately ready backend and bounded unfinished work.
    let executor = AsyncExecutor::new(ReadyBackend(Mutex::new(0)));
    let mut stage = BoundedWork::default();
    let mut operation = Operation::new(());
    let mut run = Box::pin(executor.run(&mut stage, &mut operation));
    // When: the scheduler polls the driver once.
    let outcome = run.as_mut().poll(&mut Context::from_waker(Waker::noop()));
    // Then: software regains control before another hardware interaction.
    assert!(outcome.is_pending());
    assert_eq!(*executor.backend().0.lock().expect("context lock"), 1);
}

#[tokio::test]
async fn spawned_hardware_yields_to_software_using_the_runtime_scheduler() {
    // Given: bounded hardware and software scheduled as independent Tokio tasks.
    let backend = Arc::new(ReadyBackend(Mutex::new(0)));
    let hardware_backend = Arc::clone(&backend);
    let mut tasks = tokio::task::JoinSet::new();
    tasks.spawn(async move {
        let executor = AsyncExecutor::new(hardware_backend.as_ref());
        executor
            .run(&mut BoundedWork::default(), &mut Operation::new(()))
            .await
    });
    // When: software yields once while hardware has unfinished interactions.
    tokio::task::yield_now().await;
    let observed = *backend.0.lock().expect("software context access");
    let completed = tasks
        .join_next()
        .await
        .expect("hardware task")
        .expect("hardware joins");
    // Then: software runs before all hardware interactions finish.
    assert!(
        observed > 0 && observed < 4,
        "observed {observed} hardware interactions"
    );
    assert_eq!(completed, Ok(()));
}

#[tokio::test]
async fn cancelling_terminal_yield_retains_completion_without_replaying_work() {
    // Given: one interaction remains before terminal completion.
    let executor = AsyncExecutor::new(ReadyBackend(Mutex::new(3)));
    let mut stage = BoundedWork::default();
    let mut operation = Operation::new(());
    // When: the driver is cancelled during its terminal cooperative yield and resumed.
    let mut run = Box::pin(executor.run(&mut stage, &mut operation));
    assert!(
        run.as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
    drop(run);
    let result = executor.run(&mut stage, &mut operation).await;
    // Then: the retained terminal result is delivered without another hardware step.
    assert_eq!(result, Ok(()));
    assert_eq!(*executor.backend().0.lock().expect("context lock"), 4);
}

#[derive(Debug, PartialEq, Eq)]
struct Receipt(String);

struct Retained {
    result: Option<Result<Receipt, Receipt>>,
    retired: Arc<AtomicUsize>,
}

impl Drop for Retained {
    fn drop(&mut self) {
        self.retired.fetch_add(1, Ordering::SeqCst);
    }
}

struct Terminal;

struct CallFailure(bool);

impl Stage<usize> for CallFailure {
    type Input = ();
    type Output = ();
    type Error = ();

    fn step(&mut self, count: &mut usize, input: &mut Option<()>) -> StepResult<(), ()> {
        if std::mem::take(&mut self.0) {
            return Ok(StepOutcome::Complete(Ok(())));
        }
        let Some(()) = input.take() else {
            return Ok(StepOutcome::InProgress);
        };
        *count += 1;
        self.0 = true;
        Err(RunError::BudgetExhausted)
    }
}

#[tokio::test]
async fn cancelling_terminal_yield_retains_call_error_without_replaying_work() {
    let executor = AsyncExecutor::new(ReadyBackend(Mutex::new(0)));
    let mut stage = CallFailure(false);
    let mut operation = Operation::new(());
    let mut run = Box::pin(executor.run(&mut stage, &mut operation));
    assert!(
        run.as_mut()
            .poll(&mut Context::from_waker(Waker::noop()))
            .is_pending()
    );
    drop(run);
    assert_eq!(
        executor.run(&mut stage, &mut operation).await,
        Err(AsyncRunError::Run(RunError::BudgetExhausted))
    );
    assert_eq!(*executor.backend().0.lock().expect("context lock"), 1);
    assert!(!operation.is_complete());
    assert_eq!(executor.run(&mut stage, &mut operation).await, Ok(()));
    assert!(operation.is_complete());
    assert_eq!(*executor.backend().0.lock().expect("context lock"), 1);
}

impl Stage<usize> for Terminal {
    type Input = Retained;
    type Output = Receipt;
    type Error = Receipt;

    fn step(
        &mut self,
        count: &mut usize,
        input: &mut Option<Self::Input>,
    ) -> StepResult<Receipt, Receipt> {
        let Some(mut state) = input.take() else {
            return Ok(StepOutcome::InProgress);
        };
        *count += 1;
        state
            .result
            .take()
            .map(StepOutcome::Complete)
            .ok_or(RunError::Exhausted)
    }
}

#[tokio::test]
async fn cancelled_terminal_yield_preserves_owned_success_and_error_after_state_retirement() {
    for success in [true, false] {
        // Given: a non-Clone terminal value and independently observed invocation retirement.
        let executor = AsyncExecutor::new(ReadyBackend(Mutex::new(0)));
        let retired = Arc::new(AtomicUsize::new(0));
        let receipt = Receipt("accepted request".to_owned());
        let result = if success { Ok(receipt) } else { Err(receipt) };
        let mut operation = Operation::new(Retained {
            result: Some(result),
            retired: Arc::clone(&retired),
        });
        let mut stage = Terminal;
        // When: completion commits, the driver is cancelled at yield, and then resumed.
        let mut run = Box::pin(executor.run(&mut stage, &mut operation));
        assert!(
            run.as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending()
        );
        drop(run);
        assert_eq!(retired.load(Ordering::SeqCst), 1);
        let delivered = executor.run(&mut stage, &mut operation).await;
        // Then: the original owned result is delivered once without replaying the stage.
        let expected = Receipt("accepted request".to_owned());
        assert_eq!(
            delivered,
            if success {
                Ok(expected)
            } else {
                Err(AsyncRunError::Stage(expected))
            }
        );
        assert_eq!(*executor.backend().0.lock().expect("context lock"), 1);
        assert_eq!(
            executor.run(&mut stage, &mut operation).await,
            Err(AsyncRunError::Run(RunError::Exhausted))
        );
    }
}
