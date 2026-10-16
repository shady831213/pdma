use super::{Driver, Managed, ManagementBus, RunError};
use hardware_interface_pipeline::{AsyncExecutor, HardwareDriver};
use hardware_interface_runtime::MemoryAccess;
use pdma_model_hw_components::control::MemoryError;
use std::{convert::Infallible, future::Future, sync::Arc};

#[cfg(test)]
mod tests;

impl<M: MemoryAccess + Send + Sync + 'static> Managed<M>
where
    M::Error: Into<pdma_model_dma::MemoryFailure>,
{
    /// Runs hardware and firmware until an actor fails or the caller cancels simulation.
    ///
    /// All actor futures are scoped here; cancellation leaves no spawned tasks.
    /// It does not acknowledge hardware reset or authorize memory reuse.
    pub(in crate::harness) async fn run(
        self,
    ) -> Result<Infallible, RunError<MemoryError<super::memory::SpaceError<M::Error>>>> {
        let addresses = self.shared.addresses;
        let management = self
            .firmware_cpu
            .bind_registers(ManagementBus(Arc::clone(&self.shared)));
        let firmware = Box::pin(async {
            self.firmware_runtime
                .run_main(async |registration| {
                    pdma_model_firmware::main(
                        management,
                        self.firmware_memory,
                        registration,
                        pdma_model_firmware::Configuration {
                            addresses,
                            function_count: self.function_count,
                        },
                    )
                    .await
                })
                .await
                .map_err(RunError::FirmwareInterrupt)?
                .0
                .map_err(RunError::Firmware)
        });
        let hardware = async {
            let mut pipeline = self.hardware;
            let executor = AsyncExecutor::new(Driver(self.shared));
            let mut driver = Box::new(HardwareDriver::new());
            executor
                .drive(&mut pipeline, &mut driver)
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
    pub(in crate::harness) async fn run_until<F: Future>(
        self,
        program: F,
    ) -> Result<F::Output, RunError<MemoryError<super::memory::SpaceError<M::Error>>>> {
        supervise(self.run(), program).await
    }
}

async fn supervise<S, P, E>(system: S, program: P) -> Result<P::Output, RunError<E>>
where
    S: Future<Output = Result<Infallible, RunError<E>>>,
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
