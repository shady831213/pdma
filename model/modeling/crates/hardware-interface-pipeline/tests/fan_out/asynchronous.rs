use std::{
    convert::Infallible,
    future::{Future, pending},
    sync::{Mutex, MutexGuard, PoisonError},
    task::{Context as TaskContext, Waker},
};

use hardware_interface_pipeline::{AsyncExecutor, AsyncInteractionBackend, FanOut, Operation};

use super::fixtures::{Behavior, Context, Destination, Receipt, Report, Routing, dispatch};

#[derive(Default)]
struct Backend {
    context: Mutex<Context>,
    waits: Mutex<Vec<&'static str>>,
}

fn lock<T>(value: &Mutex<T>) -> MutexGuard<'_, T> {
    value.lock().unwrap_or_else(PoisonError::into_inner)
}

impl AsyncInteractionBackend<Context, &'static str> for Backend {
    type Error = Infallible;

    async fn interact<R: Send, F>(&self, interaction: F) -> Result<R, Infallible>
    where
        F: FnOnce(&mut Context) -> R + Send,
    {
        Ok(interaction(&mut lock(&self.context)))
    }

    async fn wait(&self, wait: &'static str) -> Result<(), Infallible> {
        lock(&self.waits).push(wait);
        pending().await
    }
}

#[tokio::test]
async fn dropping_and_resuming_executor_retains_completed_receipts_and_pending_payload() {
    // Given: the real executor drives one immediate and one suspended destination.
    let executor = AsyncExecutor::new(Backend::default());
    let mut stage = FanOut::new(
        vec![
            Destination::new(0),
            Destination {
                id: 1,
                behavior: Behavior::Pending,
            },
            Destination::new(2),
        ],
        Routing,
    );
    let mut operation = Operation::new(dispatch(&[0, 1, 2]));
    let mut task_context = TaskContext::from_waker(Waker::noop());

    // When: the driving future is cancelled at its wait and resumed after release.
    let mut run = Box::pin(executor.run(&mut stage, &mut operation));
    let suspended = run.as_mut().poll(&mut task_context);
    drop(run);
    let committed = lock(&executor.backend().context).deliveries.clone();
    lock(&executor.backend().context).released = true;
    let result = executor.run(&mut stage, &mut operation).await;

    // Then: prior receipt and payload ownership survive without replaying delivery 0.
    assert!(suspended.is_pending());
    assert_eq!(committed, [(0, "payload 0".into())]);
    assert_eq!(*lock(&executor.backend().waits), ["release"]);
    assert_eq!(
        result,
        Ok(Report {
            label: "delivery".into(),
            receipts: vec![Receipt(0), Receipt(1), Receipt(2)],
        })
    );
    assert_eq!(lock(&executor.backend().context).attempts, [0, 1, 1, 2]);
    assert_eq!(lock(&executor.backend().context).deliveries.len(), 3);
    assert_eq!(lock(&executor.backend().context).plans.get(), 1);
}
