mod broadcast;
mod copy;
mod vector;

use crate::ModelResult;
use crate::dma_space::PdmaMemoryMappings;
use crate::runner::execution::command::{ExecutionCommand, ExecutionOperation};
use pdma_model_contract::abi::{AbiCommand, AbiFault, DescriptorSchemaPayloadValues};
use pdma_model_contract::binding::AbiEvent;
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
    let result: Result<ExecutionCommand, AbiFault> = match command.payload() {
        DescriptorSchemaPayloadValues::Copy(values) => copy::copy(resolution, values),
        DescriptorSchemaPayloadValues::StridedCopy(values) => copy::strided(resolution, values),
        DescriptorSchemaPayloadValues::BroadcastCopy(values) => {
            broadcast::broadcast(resolution, values)
        }
        DescriptorSchemaPayloadValues::VectorAdd(values) => vector::vector(resolution, values),
    }?;
    Ok(result.map_err(|fault| AbiEvent::new(context.queue, context.command, fault)))
}
