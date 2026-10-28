//! Hardware assembly: injected memory, device state and the DMA pipeline.

mod tests;

use crate::harness::TestResult;
use hw_model_io::memory::MemoryAccess;
use hw_model_io::memory::MemoryAccessError;
use hw_model_io::memory::SharedMemoryAccess;
use hw_model_pipeline::{AsyncExecutor, AsyncRunError, HardwareDriver};
use pdma_model_dma::{BuildError, Dma, Hardware};
use pdma_model_host::runtime::Model;

/// Assemble hardware state and the DMA pipeline over injected memory.
pub fn assemble<M: MemoryAccess>(memory: M) -> Result<(Dma<M>, Hardware), BuildError>
where
    M::Error: Into<pdma_model_dma::MemoryFailure>,
{
    let device = Dma::new(memory, 1)?;
    let hardware = device.hardware();
    Ok((device, hardware))
}

pub(crate) async fn drive(
    model: Model<SharedMemoryAccess<(), MemoryAccessError>>,
    mut hardware: Hardware,
) -> TestResult {
    let executor = AsyncExecutor::new(model);
    match executor
        .drive(&mut hardware, &mut HardwareDriver::new())
        .await
    {
        Ok(()) => Err("persistent hardware worker unexpectedly completed".into()),
        Err(AsyncRunError::Stage(error)) => Err(error.into()),
        Err(AsyncRunError::Run(error)) => Err(error.into()),
        Err(AsyncRunError::BackendInteraction(error)) => match error {},
    }
}
