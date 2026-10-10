use crate::{AsyncInteractionBackend, Operation, RunOutcome, Stage};

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

    /// Drives a retained operation to completion, awaiting each owned notification key.
    ///
    /// # Cancel safety
    /// Dropping this future releases any pending backend acquisition or wait. The caller's
    /// operation retains accepted request state and committed hardware effects remain.
    /// Calling this method again resumes that operation without reconstructing hardware.
    pub async fn run<C, S>(
        &self,
        stage: &mut S,
        operation: &mut Operation<S, C>,
    ) -> Result<S::Output, AsyncRunError<B::Error, S::Error>>
    where
        B: AsyncInteractionBackend<C, S::Wait> + Sync,
        S: Stage<C> + Send,
        S::State: Send,
        S::Output: Send,
        S::Error: Send,
        S::Wait: Send,
    {
        loop {
            if operation.is_complete() {
                return Err(AsyncRunError::AlreadyComplete);
            }
            let outcome = self
                .backend
                .interact(|context| operation.step(stage, context))
                .await
                .map_err(AsyncRunError::BackendInteraction)?
                .map_err(|_| AsyncRunError::AlreadyComplete)?;
            match outcome {
                RunOutcome::Progressed => {}
                RunOutcome::Pending(wait) => self
                    .backend
                    .wait(wait)
                    .await
                    .map_err(AsyncRunError::BackendWait)?,
                RunOutcome::Complete(Ok(output)) => return Ok(output),
                RunOutcome::Complete(Err(error)) => return Err(AsyncRunError::Stage(error)),
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
    AlreadyComplete,
}
