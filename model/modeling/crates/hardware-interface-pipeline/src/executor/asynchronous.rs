use crate::{AsyncInteractionBackend, Operation, RunError, Stage, StepOutcome};

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

    /// Drives a retained operation, yielding cooperatively after every interaction.
    ///
    /// # Cancel safety
    /// Dropping this future releases any pending backend acquisition. The caller's
    /// operation retains accepted request state and committed hardware effects remain.
    /// Calling this method again resumes that operation without reconstructing hardware.
    /// Terminal results remain in the operation until delivered after the final yield.
    pub async fn run<C, S>(
        &self,
        stage: &mut S,
        operation: &mut Operation<S, C>,
    ) -> Result<S::Output, AsyncRunError<B::Error, S::Error>>
    where
        B: AsyncInteractionBackend<C> + Sync,
        S: Stage<C> + Send,
        S::State: Send,
        S::Output: Send,
        S::Error: Send,
    {
        if let Some(result) = operation.take_completion() {
            return result.map_err(AsyncRunError::Stage);
        }
        loop {
            let outcome = self
                .backend
                .interact(|context| operation.advance(stage, context))
                .await
                .map_err(AsyncRunError::BackendInteraction)?;
            self.backend.yield_now().await;
            match outcome {
                Ok(StepOutcome::InProgress) => {}
                Ok(StepOutcome::Complete(Ok(()))) => {
                    return operation
                        .take_completion()
                        .ok_or(AsyncRunError::Run(RunError::Exhausted))?
                        .map_err(AsyncRunError::Stage);
                }
                Ok(StepOutcome::Complete(Err(error))) => match error {},
                Err(error) => return Err(AsyncRunError::Run(error)),
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
    /// The stage completed with its typed error.
    Stage(S),
    /// The retained operation rejected the driving call.
    Run(RunError),
}
