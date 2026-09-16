use super::{Driver, Managed, ManagementBus, RunError};
use hardware_interface_pipeline::{AsyncExecutor, Operation};
use hardware_interface_runtime::MemoryAccess;
use std::{convert::Infallible, future::Future, sync::Arc};

#[cfg(test)]
mod tests;

impl<M: MemoryAccess + Send + 'static> Managed<M> {
    /// Runs hardware and firmware until an actor fails or the caller cancels simulation.
    ///
    /// All actor futures are scoped here; cancellation leaves no spawned tasks.
    /// It does not acknowledge hardware reset or authorize memory reuse.
    pub async fn run(self) -> Result<Infallible, RunError> {
        let management = ManagementBus(Arc::clone(&self.shared));
        let firmware = async {
            self.firmware_cpu
                .run_main(pdma_model_firmware::main(management))
                .await
                .map_err(RunError::Firmware)
        };
        let hardware = async {
            let mut pipeline = self.pipeline;
            let executor = AsyncExecutor::new(Driver(self.shared));
            executor
                .run(&mut pipeline, &mut Operation::new(()))
                .await
                .map_err(RunError::Hardware)?;
            Err::<Infallible, _>(RunError::HardwareStopped)
        };
        supervise(hardware, firmware).await?
    }

    /// Drives the managed system for the lifetime of a caller-supplied Host execution.
    ///
    /// Actor errors take precedence over a simultaneously ready Host result.
    /// Returning or cancelling stops simulation, not an architectural reset.
    pub async fn run_until<F: Future>(self, program: F) -> Result<F::Output, RunError> {
        supervise(self.run(), program).await
    }
}

async fn supervise<S, P>(system: S, program: P) -> Result<P::Output, RunError>
where
    S: Future<Output = Result<Infallible, RunError>>,
    P: Future,
{
    tokio::select! {
        biased;
        result = system => match result {
            Ok(never) => match never {},
            Err(error) => Err(error),
        },
        result = program => Ok(result),
    }
}
