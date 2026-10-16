use super::fixtures::{Behavior, Context, Destination, Routing, Source, request};
use hardware_interface_pipeline::{
    AsyncExecutor, AsyncInteractionBackend, Dispatch, FanOut, Operation, StepResult,
};
use std::{
    convert::Infallible,
    future::Future,
    sync::{Mutex, PoisonError},
    task::{Context as TaskContext, Waker},
};
struct Backend(Mutex<Context>);
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
        Ok(interaction(
            &mut self.0.lock().unwrap_or_else(PoisonError::into_inner),
        ))
    }
}
#[tokio::test]
async fn cancelled_executor_resumes_stalled_child_without_restarting_source() {
    // Given: the real executor reaches a stalled destination.
    let executor = AsyncExecutor::new(Backend(Mutex::new(Context::default())));
    let mut hardware = FanOut::new(
        Source,
        Dispatch::new(Routing::Accept),
        vec![Destination {
            id: 0,
            behavior: Behavior::Stall,
            pending: (None, false),
        }],
    );
    let mut operation = Operation::new(request(0));
    let mut context = TaskContext::from_waker(Waker::noop());
    // When: the driving future is dropped after output acceptance, then resumed.
    let mut future = Box::pin(executor.run(&mut hardware, &mut operation));
    for _ in 0..3 {
        assert!(future.as_mut().poll(&mut context).is_pending());
    }
    drop(future);
    executor
        .backend()
        .0
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .released = true;
    let result = executor.run(&mut hardware, &mut operation).await;
    // Then: original input ownership survives and the source is not replayed.
    assert_eq!(result, Ok(vec![(0, ())]));
    let hardware_context = executor
        .backend()
        .0
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    assert_eq!(hardware_context.source_calls, 1);
    assert_eq!(hardware_context.deliveries, [(0, "owned payload".into())]);
    drop(hardware_context);
}
