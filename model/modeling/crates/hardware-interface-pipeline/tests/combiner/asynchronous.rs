use std::{
    convert::Infallible,
    future::{Future, pending},
    sync::{Mutex, MutexGuard, PoisonError},
    task::{Context as TaskContext, Waker},
};

use hardware_interface_pipeline::{
    AsyncExecutor, AsyncInteractionBackend, Combiner, Operation, Selection, StepOutcome,
};

use super::sources::{Context, Job, Queue};

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
async fn executor_drop_and_resume_retains_idle_search_and_active_grant() {
    // Given: a real async executor with an empty source that suspends after accepting input.
    let executor = AsyncExecutor::new(Backend::default());
    let mut hardware = Combiner::new(
        vec![
            Queue {
                id: 0,
                delayed: true,
            },
            Queue::new(1),
        ],
        Selection::RoundRobin,
        "queue changed",
    );
    let mut operation = Operation::new(());
    let mut task_context = TaskContext::from_waker(Waker::noop());

    // When: driving futures are dropped at idle and active waits, then resumed after release.
    let mut idle_run = Box::pin(executor.run(&mut hardware, &mut operation));
    let idle_poll = idle_run.as_mut().poll(&mut task_context);
    drop(idle_run);
    lock(&executor.backend().context).enqueue(0, [Job::Value(70)]);
    lock(&executor.backend().context).enqueue(1, [Job::Value(80)]);
    let mut active_run = Box::pin(executor.run(&mut hardware, &mut operation));
    let active_poll = active_run.as_mut().poll(&mut task_context);
    drop(active_run);
    lock(&executor.backend().context).released = true;
    let result = executor.run(&mut hardware, &mut operation).await;

    // Then: backend waits receive both keys and the accepted source completes exactly once.
    assert!(idle_poll.is_pending());
    assert!(active_poll.is_pending());
    assert_eq!(
        *lock(&executor.backend().waits),
        ["queue changed", "source release"]
    );
    assert_eq!(result, Ok(70));
    assert_eq!(lock(&executor.backend().context).consumed, [0]);
    assert_eq!(
        *lock(&executor.backend().context).probes.borrow(),
        [0, 1, 0]
    );
}
