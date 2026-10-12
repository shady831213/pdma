use super::fixtures::{Context, Destination, Receipt, Routing, Source, request};
use hardware_interface_pipeline::{
    AsyncExecutor, AsyncInteractionBackend, FanOut, Operation, StepResult,
};
use std::{
    convert::Infallible,
    future::Future,
    sync::{Mutex, MutexGuard, PoisonError},
    task::{Context as TaskContext, Waker},
};
struct Backend(Mutex<Context>);
fn lock<T>(value: &Mutex<T>) -> MutexGuard<'_, T> {
    value.lock().unwrap_or_else(PoisonError::into_inner)
}
impl AsyncInteractionBackend<Context> for Backend {
    type Error = Infallible;
    async fn yield_now(&self) {
        tokio::task::yield_now().await;
    }
    async fn interact<O: Send, E: Send, F>(
        &self,
        interaction: F,
    ) -> Result<StepResult<O, E>, Infallible>
    where
        F: FnOnce(&mut Context) -> StepResult<O, E> + Send,
    {
        Ok(interaction(&mut lock(&self.0)))
    }
}
#[tokio::test]
async fn dropping_and_resuming_executor_retains_completed_receipts_and_stalled_payload() {
    // Given: one output stalls while both peers can commit.
    let executor = AsyncExecutor::new(Backend(Mutex::new(Context {
        blocked: vec![1],
        ..Context::default()
    })));
    let mut hardware = FanOut::new(Source, Routing, (0..3).map(Destination::new).collect());
    let mut operation = Operation::new(request(&[0, 1, 2]));
    let mut task_context = TaskContext::from_waker(Waker::noop());
    // When: cancellation occurs after the first output sweep, then the caller resumes.
    let mut run = Box::pin(executor.run(&mut hardware, &mut operation));
    for _ in 0..3 {
        assert!(run.as_mut().poll(&mut task_context).is_pending());
    }
    drop(run);
    let committed = lock(&executor.backend().0).deliveries.clone();
    lock(&executor.backend().0).blocked.clear();
    let result = executor.run(&mut hardware, &mut operation).await;
    // Then: earlier receipts and the unfinished payload survive without replay.
    assert_eq!(
        committed,
        [(0, "payload 0".into()), (2, "payload 2".into())]
    );
    assert_eq!(
        result,
        Ok(vec![(0, Receipt(0)), (2, Receipt(2)), (1, Receipt(1))])
    );
    assert_eq!(lock(&executor.backend().0).sources, 1);
    assert_eq!(lock(&executor.backend().0).deliveries.len(), 3);
}
