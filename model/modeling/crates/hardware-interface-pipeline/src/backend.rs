use std::future::Future;

use crate::StepOutcome;

/// Provides asynchronous, closure-scoped access to a pipeline context.
pub trait AsyncInteractionBackend<C, W> {
    /// An interaction or wait failure reported by the backend.
    type Error: Send;

    /// Runs one synchronous interaction while the backend owns context access.
    fn interact<O, E, F>(
        &self,
        interaction: F,
    ) -> impl Future<Output = Result<StepOutcome<O, E, W>, Self::Error>> + Send
    where
        O: Send,
        E: Send,
        W: Send,
        F: FnOnce(&mut C) -> StepOutcome<O, E, W> + Send;

    /// Waits for a change represented by an owned stage wait value.
    fn wait(&self, wait: W) -> impl Future<Output = Result<(), Self::Error>> + Send
    where
        W: Send;
}

impl<C, W, B> AsyncInteractionBackend<C, W> for &B
where
    B: AsyncInteractionBackend<C, W> + Sync + ?Sized,
{
    type Error = B::Error;

    fn interact<O, E, F>(
        &self,
        interaction: F,
    ) -> impl Future<Output = Result<StepOutcome<O, E, W>, Self::Error>> + Send
    where
        O: Send,
        E: Send,
        W: Send,
        F: FnOnce(&mut C) -> StepOutcome<O, E, W> + Send,
    {
        B::interact(*self, interaction)
    }

    fn wait(&self, wait: W) -> impl Future<Output = Result<(), Self::Error>> + Send
    where
        W: Send,
    {
        B::wait(*self, wait)
    }
}
