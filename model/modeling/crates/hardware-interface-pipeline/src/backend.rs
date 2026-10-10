use std::future::Future;

/// Provides asynchronous, closure-scoped access to a pipeline context.
pub trait AsyncInteractionBackend<C, W> {
    /// An interaction or wait failure reported by the backend.
    type Error: Send;

    /// Runs one synchronous interaction while the backend owns context access.
    fn interact<R, F>(&self, interaction: F) -> impl Future<Output = Result<R, Self::Error>> + Send
    where
        R: Send,
        F: FnOnce(&mut C) -> R + Send;

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

    fn interact<R, F>(&self, interaction: F) -> impl Future<Output = Result<R, Self::Error>> + Send
    where
        R: Send,
        F: FnOnce(&mut C) -> R + Send,
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
