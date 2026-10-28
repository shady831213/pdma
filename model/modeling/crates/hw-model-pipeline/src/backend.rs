use std::future::Future;

use crate::StepResult;

/// Provides asynchronous, closure-scoped access to a pipeline context.
pub trait AsyncInteractionBackend<C> {
    /// An interaction failure reported by the backend.
    type Error: Send;

    /// Runs one synchronous interaction while the backend owns context access.
    fn interact<O, E, F>(
        &self,
        interaction: F,
    ) -> impl Future<Output = Result<StepResult<O, E>, Self::Error>> + Send
    where
        O: Send,
        E: Send,
        F: FnOnce(&mut C) -> StepResult<O, E> + Send;

    /// Yields cooperatively through the backend's asynchronous scheduler.
    fn yield_now(&self) -> impl Future<Output = ()> + Send;
}

impl<C, B> AsyncInteractionBackend<C> for &B
where
    B: AsyncInteractionBackend<C> + Sync + ?Sized,
{
    type Error = B::Error;

    fn interact<O, E, F>(
        &self,
        interaction: F,
    ) -> impl Future<Output = Result<StepResult<O, E>, Self::Error>> + Send
    where
        O: Send,
        E: Send,
        F: FnOnce(&mut C) -> StepResult<O, E> + Send,
    {
        B::interact(*self, interaction)
    }

    fn yield_now(&self) -> impl Future<Output = ()> + Send {
        B::yield_now(*self)
    }
}
