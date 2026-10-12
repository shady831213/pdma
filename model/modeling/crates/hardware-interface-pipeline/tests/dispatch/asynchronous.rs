use std::{
    convert::Infallible,
    future::{Future, pending},
    sync::{Mutex, MutexGuard, PoisonError},
    task::{Context as TaskContext, Waker},
};

use hardware_interface_pipeline::{
    AsyncExecutor, AsyncInteractionBackend, Dispatch, Operation, StepOutcome,
};

use super::fixtures::{Behavior, Context, Destination, Routing, request};

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

    async fn interact<O: Send, E: Send, F>(
        &self,
        interaction: F,
    ) -> Result<StepOutcome<O, E, &'static str>, Infallible>
    where
        F: FnOnce(&mut Context) -> StepOutcome<O, E, &'static str> + Send,
    {
        Ok(interaction(&mut lock(&self.context)))
    }

    async fn wait(&self, wait: &'static str) -> Result<(), Infallible> {
        lock(&self.waits).push(wait);
        pending().await
    }
}

#[tokio::test]
async fn cancelled_executor_resumes_pending_child_without_rerouting() {
    // Given: the real executor reaches a wait on the selected destination.
    let executor = AsyncExecutor::new(Backend::default());
    let mut hardware = Dispatch::new(
        vec![
            Destination::new(0),
            Destination {
                id: 1,
                behavior: Behavior::Pending,
            },
        ],
        Routing::Accept,
    );
    let mut operation = Operation::new(request(1));
    let mut task_context = TaskContext::from_waker(Waker::noop());

    // When: the driving future is dropped at the wait, then resumed after release.
    let mut run = Box::pin(executor.run(&mut hardware, &mut operation));
    let suspended = run.as_mut().poll(&mut task_context);
    drop(run);
    let deliveries_before_release = lock(&executor.backend().context).deliveries.clone();
    lock(&executor.backend().context).released = true;
    let result = executor.run(&mut hardware, &mut operation).await;

    // Then: cancellation preserves payload ownership and never invokes routing again.
    assert!(suspended.is_pending());
    assert!(deliveries_before_release.is_empty());
    assert_eq!(*lock(&executor.backend().waits), ["release"]);
    assert_eq!(result, Ok(()));
    let context = lock(&executor.backend().context);
    assert_eq!(context.deliveries, [(1, "routed owned payload".into())]);
    assert_eq!(context.attempts, [1, 1]);
    assert_eq!(context.routes.get(), 1);
    drop(context);
}
