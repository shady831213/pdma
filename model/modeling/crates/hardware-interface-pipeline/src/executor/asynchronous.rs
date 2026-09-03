use crate::{AsyncInteractionBackend, Stage, StepOutcome};

/// A statically dispatched asynchronous pipeline executor.
#[derive(Debug)]
pub struct AsyncExecutor<B> {
    backend: B,
}

impl<B> AsyncExecutor<B> {
    /// Creates an executor that owns its interaction backend.
    pub const fn new(backend: B) -> Self {
        Self { backend }
    }

    /// Returns a shared reference to the interaction backend.
    pub const fn backend(&self) -> &B {
        &self.backend
    }

    /// Runs a stage to terminal completion, waiting at each owned pending boundary.
    pub async fn run<C, S>(
        &self,
        stage: &mut S,
    ) -> Result<S::Output, AsyncRunError<B::Error, S::Error>>
    where
        B: AsyncInteractionBackend<C, S::Wait> + Sync,
        S: Stage<C> + Send,
        S::Output: Send,
        S::Error: Send,
        S::Wait: Send,
    {
        loop {
            let outcome = self
                .backend
                .interact(|context| stage.step(context))
                .await
                .map_err(AsyncRunError::BackendInteraction)?;
            match outcome {
                StepOutcome::Progressed => {}
                StepOutcome::Pending(wait) => self
                    .backend
                    .wait(wait)
                    .await
                    .map_err(AsyncRunError::BackendWait)?,
                StepOutcome::Complete(Ok(output)) => return Ok(output),
                StepOutcome::Complete(Err(error)) => return Err(AsyncRunError::Stage(error)),
                StepOutcome::Exhausted => return Err(AsyncRunError::Exhausted),
            }
        }
    }
}

/// A typed asynchronous pipeline execution failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use = "an asynchronous pipeline error must be handled"]
pub enum AsyncRunError<B, S> {
    /// Context interaction failed before or after closure-scoped access.
    BackendInteraction(B),
    /// Waiting on an owned pending value failed.
    BackendWait(B),
    /// The stage completed with its typed error.
    Stage(S),
    /// The pipeline was already terminal before the executor call.
    Exhausted,
}
