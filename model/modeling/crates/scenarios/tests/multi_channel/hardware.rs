//! Hardware receives memory and contains no software registration or CPU state.

use hardware_interface_pipeline::{AsyncExecutor, AsyncRunError, Operation};
use hardware_interface_runtime::SharedMemoryAccess;
use pdma_model_dma::{CommandLoop, Dma};
use pdma_model_host::runtime::Model;

use crate::TestResult;

pub(super) fn assemble(
    memory: SharedMemoryAccess,
) -> TestResult<(Model<SharedMemoryAccess>, CommandLoop)> {
    let device = Dma::new(memory, 3)?;
    let stage = device.command_loop();
    Ok((Model::new(device), stage))
}

pub(super) async fn drive(model: Model<SharedMemoryAccess>, mut stage: CommandLoop) -> TestResult {
    let executor = AsyncExecutor::new(model);
    match executor.run(&mut stage, &mut Operation::new(())).await {
        Ok(()) => Err("sustained hardware worker unexpectedly completed".into()),
        Err(AsyncRunError::Stage(error)) => Err(error.into()),
        Err(AsyncRunError::AlreadyComplete) => Err("hardware operation already complete".into()),
        Err(AsyncRunError::BackendInteraction(error) | AsyncRunError::BackendWait(error)) => {
            match error {}
        }
    }
}
