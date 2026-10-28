use crate::{AsyncInteractionBackend, HardwareDriver, Operation, RunError, Stage, StepOutcome};

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

    /// Drives finite hardware services, yielding after every bounded interaction.
    ///
    /// # Cancel safety
    /// Hardware retains accepted work; the caller's driver retains pending input
    /// and terminal errors across cancellation, including the final yield.
    /// Completed service effects are never replayed on resumption.
    pub async fn drive<C, S>(
        &self,
        hardware: &mut S,
        driver: &mut HardwareDriver<S, C>,
    ) -> Result<(), AsyncRunError<B::Error, S::Error>>
    where
        B: AsyncInteractionBackend<C> + Sync,
        S: Stage<C, Input = (), Output = ()> + Send,
        S::Error: Send,
    {
        loop {
            if let Some(failure) = driver.take_failure() {
                return Err(match failure {
                    Ok(error) => AsyncRunError::Stage(error),
                    Err(error) => AsyncRunError::Run(error),
                });
            }
            if driver.is_complete() {
                return Err(AsyncRunError::Run(RunError::Exhausted));
            }
            let outcome = self
                .backend
                .interact(|context| driver.advance(hardware, context))
                .await
                .map_err(AsyncRunError::BackendInteraction)?;
            self.backend.yield_now().await;
            match outcome {
                Ok(StepOutcome::InProgress) => {}
                Ok(StepOutcome::Complete(Ok(()))) => {
                    return Err(match driver.take_failure() {
                        Some(Ok(error)) => AsyncRunError::Stage(error),
                        Some(Err(error)) => AsyncRunError::Run(error),
                        None => AsyncRunError::Run(RunError::Exhausted),
                    });
                }
                Ok(StepOutcome::Complete(Err(error))) => match error {},
                Err(error) => return Err(AsyncRunError::Run(error)),
            }
        }
    }

    /// Drives a retained operation, yielding cooperatively after every interaction.
    ///
    /// # Cancel safety
    /// Dropping this future releases any pending backend acquisition. The caller's
    /// operation retains pending input; hardware retains accepted work and committed effects.
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
        S::Input: Send,
        S::Output: Send,
        S::Error: Send,
    {
        if let Some(error) = operation.take_run_error() {
            return Err(AsyncRunError::Run(error));
        }
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
                    if let Some(error) = operation.take_run_error() {
                        return Err(AsyncRunError::Run(error));
                    }
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

impl<B: std::fmt::Display, S: std::fmt::Display> std::fmt::Display for AsyncRunError<B, S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BackendInteraction(error) => {
                write!(f, "pipeline backend interaction failed: {error}")
            }
            Self::Stage(error) => write!(f, "pipeline stage failed: {error}"),
            Self::Run(error) => error.fmt(f),
        }
    }
}

impl<B, S> std::error::Error for AsyncRunError<B, S>
where
    B: std::error::Error + 'static,
    S: std::error::Error + 'static,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::BackendInteraction(error) => Some(error),
            Self::Stage(error) => Some(error),
            Self::Run(error) => Some(error),
        }
    }
}
