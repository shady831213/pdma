//! Hardware assembly: injected memory, device state and the existing stage graph.

mod tests;

use crate::harness::TestResult;
use hardware_interface_pipeline::{AsyncExecutor, AsyncRunError, Operation};
use hardware_interface_runtime::MemoryAccess;
use hardware_interface_runtime::SharedMemoryAccess;
use pdma_model_device::{BuildError, CommandLoop, Device};
use pdma_model_host::runtime::Model;

/// Assemble the existing hardware state and stage graph over injected memory.
pub fn assemble<M: MemoryAccess>(memory: M) -> Result<(Device<M>, CommandLoop), BuildError> {
    let device = Device::new(memory, 1)?;
    let command_loop = device.command_loop();
    Ok((device, command_loop))
}

pub(crate) async fn drive(model: Model<SharedMemoryAccess>, mut stage: CommandLoop) -> TestResult {
    let executor = AsyncExecutor::new(model);
    match executor.run(&mut stage, &mut Operation::new(())).await {
        Ok(()) => Err("persistent hardware worker unexpectedly completed".into()),
        Err(AsyncRunError::Stage(error)) => Err(error.into()),
        Err(AsyncRunError::Exhausted) => Err("fresh hardware operation exhausted".into()),
        Err(AsyncRunError::BackendInteraction(error) | AsyncRunError::BackendWait(error)) => {
            match error {}
        }
    }
}
