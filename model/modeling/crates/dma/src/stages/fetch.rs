use crate::channel::Accepted;
use crate::{CommandSnapshot, Error, ExecutionContext, FetchedCommand};
use hardware_interface_pipeline::{Stage, StepOutcome, StepResult};
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
    fn step(
        &mut self,
        context: &mut ExecutionContext<'_, M, SB>,
        input: &mut Option<Self::Input>,
    ) -> StepResult<Self::Output, Error> {
        let Some(input) = input.take() else {
            return Ok(StepOutcome::InProgress);
        };
        let Some(snapshot) = input else {
            return Ok(StepOutcome::Complete(Ok(None)));
        };
        if snapshot.work.channel != context.id {
            return Ok(StepOutcome::Complete(Err(Error::Routing)));
        }
        let work = snapshot.work;
        let aborting = context.abort_context.is_some();
        let channel = match context.channel(work) {
            Ok(channel) => channel,
            Err(error) => return Ok(StepOutcome::Complete(Err(error))),
        };
        if !channel.enabled()
            || channel.work.is_some()
            || channel.command.pointers().consumer != snapshot.consumer
            || channel.command.entry(snapshot.consumer).ok() != Some(snapshot.address)
            || channel.command.pointers().is_empty()
            || (!aborting && channel.response.pointers().pending() >= channel.response.capacity())
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
        match context.selected.command.peek(&memory) {
            Ok(Some(command)) => {
                if context
                    .abort_context
                    .is_some_and(|target| crate::references(&command).contains(&Some(target)))
                {
                    if let Err(fatal) = context.selected.command.consume() {
                        context.selected.latch(fatal);
                        context.changed();
                        return Ok(StepOutcome::Complete(Err(Error::ChannelFatal(fatal))));
                    }
                    context.changed();
                    return Ok(StepOutcome::Complete(Ok(None)));
                }
                if context.selected.response.pointers().pending()
                    >= context.selected.response.capacity()
                {
                    return Ok(StepOutcome::Complete(Ok(None)));
                }
                if let Err(fatal) = context.selected.command.consume() {
                    context.selected.latch(fatal);
                    context.changed();
                    return Ok(StepOutcome::Complete(Err(Error::ChannelFatal(fatal))));
                }
                context.selected.work = Some(Accepted {
                    command,
                    unread: None,
                    payload: None,
                    completion: None,
                    progress: None,
                });
                *context.active = Some(work);
                context.generation = work.generation;
                context.changed();
                Ok(StepOutcome::Complete(Ok(Some(FetchedCommand {
                    work,
                    command,
                }))))
            }
            Ok(None) => Ok(StepOutcome::Complete(Ok(None))),
            Err(fatal) => {
                context.selected.latch(fatal);
                context.changed();
                Ok(StepOutcome::Complete(Err(Error::ChannelFatal(fatal))))
            }
        }
    }
}
