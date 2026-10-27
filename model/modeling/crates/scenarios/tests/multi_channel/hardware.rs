//! Hardware receives memory and contains no software registration or CPU state.

use hardware_interface_io::memory::MemoryAccessError;
use hardware_interface_io::memory::SharedMemoryAccess;
use hardware_interface_pipeline::{AsyncExecutor, AsyncRunError, HardwareDriver};
use pdma_model_dma::{Dma, Hardware};
use pdma_model_host::runtime::Model;

use crate::TestResult;

pub(super) fn assemble(
    memory: SharedMemoryAccess<(), MemoryAccessError>,
) -> TestResult<(Model<SharedMemoryAccess<(), MemoryAccessError>>, Hardware)> {
    let device = Dma::new(memory, 3)?;
    let hardware = device.hardware();
    Ok((Model::new(device), hardware))
}

pub(super) async fn drive(
    model: Model<SharedMemoryAccess<(), MemoryAccessError>>,
    mut hardware: Hardware,
) -> TestResult {
    let executor = AsyncExecutor::new(model);
    match executor
        .drive(&mut hardware, &mut HardwareDriver::new())
        .await
    {
        Ok(()) => Err("sustained hardware worker unexpectedly completed".into()),
        Err(AsyncRunError::Stage(error)) => Err(error.into()),
        Err(AsyncRunError::Run(error)) => Err(error.into()),
        Err(AsyncRunError::BackendInteraction(error)) => match error {},
    }
}
