use crate::channel::Accepted;
use crate::{CommandSnapshot, Error, ExecutionContext, FetchedCommand};
use hardware_interface_pipeline::{RunError, Stage, StepOutcome, StepResult};
use hardware_interface_runtime::{BoundMemory, MemoryAccess};
use pdma_model_contract::dma::{ContextId, DmaAccess};
use pdma_model_contract::ring::Ring;

/// Fetches only a fresh selected descriptor and atomically retains acceptance.
#[derive(Debug, Default)]
pub struct Fetch;

impl<M: MemoryAccess<SB>, SB> Stage<ExecutionContext<'_, M, SB>> for Fetch {
    type Input = Option<CommandSnapshot>;
    type Output = Option<FetchedCommand>;
    type Error = Error;
    type State = Option<Self::Input>;
    fn start(input: Self::Input) -> Self::State {
        Some(input)
    }
    fn step(
        &mut self,
        context: &mut ExecutionContext<'_, M, SB>,
        state: &mut Self::State,
    ) -> StepResult<Self::Output, Error> {
        let Some(snapshot) = state.take().ok_or(RunError::Exhausted)? else {
            return Ok(StepOutcome::Complete(Ok(None)));
        };
        if snapshot.work.channel != context.id {
            return Ok(StepOutcome::Complete(Err(Error::Routing)));
        }
        let work = crate::Work {
            channel: context.id,
            generation: context.selected.generation,
        };
        let channel = match context.channel(work) {
            Ok(channel) => channel,
            Err(error) => return Ok(StepOutcome::Complete(Err(error))),
        };
        let snapshot =
            if work == snapshot.work && channel.command.pointers().consumer == snapshot.consumer {
                snapshot
            } else {
                CommandSnapshot {
                    work,
                    consumer: channel.command.pointers().consumer,
                    address: channel
                        .command
                        .entry(channel.command.pointers().consumer)
                        .map_err(|_| RunError::Exhausted)?,
                }
            };
        if !channel.enabled()
            || channel.work.is_some()
            || channel.command.entry(snapshot.consumer).ok() != Some(snapshot.address)
            || channel.command.pointers().is_empty()
            || channel.response.pointers().pending() >= channel.response.capacity()
            || context.active.is_some()
        {
            return Ok(StepOutcome::Complete(Ok(None)));
        }
        let memory = BoundMemory::new(
            context.memory,
            (context.sideband)(DmaAccess {
                channel: work.channel,
                context: ContextId::new(context.selected.registers.values.ring_context.raw()),
            }),
        );
        match context.selected.command.pop(&memory) {
            Ok(Some(command)) => {
                context.selected.work = Some(Accepted {
                    command,
                    continuation: None,
                    progress: None,
                });
                *context.active = Some(work);
                Ok(StepOutcome::Complete(Ok(Some(FetchedCommand {
                    work,
                    command,
                }))))
            }
            Ok(None) => Ok(StepOutcome::Complete(Ok(None))),
            Err(fatal) => {
                context.selected.latch(fatal);
                Ok(StepOutcome::Complete(Err(Error::ChannelFatal(fatal))))
            }
        }
    }
}
