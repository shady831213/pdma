//! Hardware receives memory and contains no software registration or CPU state.

use hardware_interface_pipeline::{AsyncExecutor, AsyncRunError, HardwareDriver};
use hardware_interface_runtime::SharedMemoryAccess;
use pdma_model_dma::{Dma, Service};
use pdma_model_host::runtime::Model;

use crate::TestResult;

pub(super) fn assemble(
    memory: SharedMemoryAccess,
) -> TestResult<(Model<SharedMemoryAccess>, Service<SharedMemoryAccess>)> {
    let device = Dma::new(memory, 3)?;
    let stage = device.service();
    Ok((Model::new(device), stage))
}

pub(super) async fn drive(
    model: Model<SharedMemoryAccess>,
    mut stage: Service<SharedMemoryAccess>,
) -> TestResult {
    let executor = AsyncExecutor::new(model);
    match executor.drive(&mut stage, &mut HardwareDriver::new()).await {
        Ok(()) => Err("sustained hardware worker unexpectedly completed".into()),
        Err(AsyncRunError::Stage(error)) => Err(error.into()),
        Err(AsyncRunError::Run(error)) => Err(error.into()),
        Err(AsyncRunError::BackendInteraction(error)) => match error {},
    }
}
