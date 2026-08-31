mod broadcast;
mod copy;
mod resolve;
mod vector;

use crate::ModelResult;
use crate::abi::dma::PdmaMemoryMappings;
use pdma_model_contract::abi::{AbiCommand, AbiCommandPayloadValues, AbiEvent, AbiFault};
use pdma_model_contract::semantic::{Command, CommandId, CommandOperation};

#[derive(Clone, Copy)]
pub(crate) struct ExactCommandContext {
    queue: u64,
    exact_command: u64,
    command: CommandId,
}

impl ExactCommandContext {
    pub(crate) const fn new(queue: u64, exact_command: u64, command: CommandId) -> Self {
        Self {
            queue,
            exact_command,
            command,
        }
    }
}

#[derive(Clone, Copy)]
struct ResolutionContext<'a> {
    dma: &'a PdmaMemoryMappings,
    command: CommandId,
}

impl ResolutionContext<'_> {
    const fn complete(self, operation: CommandOperation) -> Command {
        Command::new(self.command, operation)
    }
}

pub(crate) fn resolve(
    dma: &PdmaMemoryMappings,
    context: ExactCommandContext,
    command: AbiCommand,
) -> ModelResult<Result<Command, AbiEvent>> {
    let resolution = ResolutionContext {
        dma,
        command: context.command,
    };
    let result: Result<Command, AbiFault> = match command.payload() {
        AbiCommandPayloadValues::Copy(values) => copy::copy(resolution, values),
        AbiCommandPayloadValues::StridedCopy(values) => copy::strided(resolution, values),
        AbiCommandPayloadValues::BroadcastCopy(values) => broadcast::broadcast(resolution, values),
        AbiCommandPayloadValues::VectorAdd(values) => vector::vector(resolution, values),
    }?;
    Ok(result.map_err(|fault| AbiEvent::new(context.queue, context.exact_command, fault)))
}
