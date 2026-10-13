//! Hardware assembly: injected memory, device state and the existing stage graph.

mod tests;

use crate::harness::TestResult;
use hardware_interface_pipeline::{AsyncExecutor, AsyncRunError, HardwareDriver};
use hardware_interface_runtime::MemoryAccess;
use hardware_interface_runtime::SharedMemoryAccess;
use pdma_model_dma::{BuildError, Dma, Service};
use pdma_model_host::runtime::Model;

/// Assemble the existing hardware state and stage graph over injected memory.
pub fn assemble<M: MemoryAccess>(memory: M) -> Result<(Dma<M>, Service<M>), BuildError>
where
    M::Error: Into<pdma_model_dma::MemoryFailure>,
{
    let device = Dma::new(memory, 1)?;
    let service = device.service();
    Ok((device, service))
}

pub(crate) async fn drive(
    model: Model<SharedMemoryAccess>,
    mut stage: Service<SharedMemoryAccess>,
) -> TestResult {
    let executor = AsyncExecutor::new(model);
    match executor.drive(&mut stage, &mut HardwareDriver::new()).await {
        Ok(()) => Err("persistent hardware worker unexpectedly completed".into()),
        Err(AsyncRunError::Stage(error)) => Err(error.into()),
        Err(AsyncRunError::Run(error)) => Err(error.into()),
        Err(AsyncRunError::BackendInteraction(error)) => match error {},
    }
}
