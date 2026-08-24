mod broadcast;
mod copy;
mod vector;

use crate::ModelResult;
use crate::dma_space::PdmaMemoryMappings;
use crate::runner::native::execution::{ExecutionCommand, ExecutionOperation};
use pdma_model_contract::abi::{AbiCommand, AbiFault};
use pdma_model_contract::bridge::AbiEvent;
use pdma_model_contract::semantic::{CommandId, CompletionRingId, QueueId};

#[derive(Clone, Copy)]
pub(super) struct ExactCommandContext {
    queue: QueueId,
    completion: CompletionRingId,
    command: CommandId,
}

impl ExactCommandContext {
    pub(super) const fn new(
        queue: QueueId,
        completion: CompletionRingId,
        command: CommandId,
    ) -> Self {
        Self {
            queue,
            completion,
            command,
        }
    }
}

#[derive(Clone, Copy)]
struct ResolutionContext<'a> {
    dma: &'a PdmaMemoryMappings,
    completion: CompletionRingId,
    command: CommandId,
}

impl ResolutionContext<'_> {
    const fn complete(self, operation: ExecutionOperation) -> ExecutionCommand {
        ExecutionCommand::new(self.command, self.completion, operation)
    }
}

pub(super) fn resolve(
    dma: &PdmaMemoryMappings,
    context: ExactCommandContext,
    command: AbiCommand,
) -> ModelResult<Result<ExecutionCommand, AbiEvent>> {
    let resolution = ResolutionContext {
        dma,
        completion: context.completion,
        command: context.command,
    };
    let result: Result<ExecutionCommand, AbiFault> = match command {
        AbiCommand::Copy(command) => copy::copy(resolution, command),
        AbiCommand::StridedCopy(command) => copy::strided(resolution, command),
        AbiCommand::BroadcastCopy(command) => broadcast::broadcast(resolution, command),
        AbiCommand::VectorAdd(command) => vector::vector(resolution, command),
    }?;
    Ok(result.map_err(|fault| AbiEvent::new(context.queue, context.command, fault)))
}
