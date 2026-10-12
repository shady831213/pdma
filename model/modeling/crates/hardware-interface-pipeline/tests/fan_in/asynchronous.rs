use super::{
    policy::Policy,
    sources::{Accept, Candidate, Context, Job},
};
use hardware_interface_pipeline::{
    AsyncExecutor, AsyncInteractionBackend, FanIn, Operation, StepResult,
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
async fn canceled_gather_resumes_completed_candidates_and_active_output() {
    // Given: one accepted output is stalled after the input sweep.
    let mut hardware_context = Context::with_jobs(&[0, 1]);
    hardware_context.blocked.push(0);
    let executor = AsyncExecutor::new(Backend(Mutex::new(hardware_context)));
    let mut hardware = FanIn::new(
        (0..2).map(Candidate).collect(),
        Policy::Fixed,
        Accept::Normal,
    );
    let mut operation = Operation::new(vec![(); 2]);
    let mut task_context = TaskContext::from_waker(Waker::noop());
    // When: cancellation follows acceptance and the caller later resumes.
    let mut future = Box::pin(executor.run(&mut hardware, &mut operation));
    for _ in 0..6 {
        assert!(future.as_mut().poll(&mut task_context).is_pending());
    }
    drop(future);
    executor
        .backend()
        .0
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .blocked
        .clear();
    let result = executor.run(&mut hardware, &mut operation).await;
    // Then: no source or output acceptance is replayed.
    assert_eq!(
        result,
        Ok(vec![(
            0,
            Some(Job {
                channel: 0,
                value: 7
            })
        )])
    );
    let context = executor
        .backend()
        .0
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    assert_eq!(context.observed, [0, 1]);
    assert_eq!(context.accepted, [0]);
    drop(context);
}
#[tokio::test]
async fn executor_drop_before_planning_retains_collected_values() {
    // Given: cancellation immediately follows completed input collection.
    let executor = AsyncExecutor::new(Backend(Mutex::new(Context::with_jobs(&[1]))));
    let mut hardware = FanIn::new(
        (0..2).map(Candidate).collect(),
        Policy::Fixed,
        Accept::Normal,
    );
    let mut operation = Operation::new(vec![(); 2]);
    let mut task_context = TaskContext::from_waker(Waker::noop());
    // When: a new future resumes the same invocation.
    let mut future = Box::pin(executor.run(&mut hardware, &mut operation));
    assert!(future.as_mut().poll(&mut task_context).is_pending());
    drop(future);
    let result = executor.run(&mut hardware, &mut operation).await;
    // Then: already completed candidate values are neither recollected nor lost.
    assert_eq!(
        result,
        Ok(vec![(
            0,
            Some(Job {
                channel: 1,
                value: 7
            })
        )])
    );
    let context = executor
        .backend()
        .0
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    assert_eq!(context.observed, [0, 1]);
    assert_eq!(context.accepted, [1]);
    drop(context);
}
