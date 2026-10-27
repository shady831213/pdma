use crate::SelectedResources;
use crate::channel::Accepted;
use crate::{CommandAdmission, CommandRequest, CommandSnapshot, Dma, Error, FetchedCommand, Flow};
use hardware_interface_memory::{BoundMemory, MemoryAccess};
use hardware_interface_pipeline::{Stage, StepOutcome, StepResult};
use pdma_model_contract::dma::{ContextId, DmaAccess};
use pdma_model_contract::ring::Ring;

/// Accepts fresh or resumed work and transfers its descriptor into execution dataflow.
#[derive(Debug, Default)]
pub struct Fetch;

impl<M: MemoryAccess<SB>, SB> Stage<Dma<M, SB>> for Fetch {
    type Input = Option<CommandRequest>;
    type Output = Flow<FetchedCommand>;
    type Error = std::convert::Infallible;
    fn step(
        &mut self,
        dma: &mut Dma<M, SB>,
        input: &mut Option<Self::Input>,
    ) -> StepResult<Self::Output, Self::Error> {
        let Some(request) = input.take() else {
            return Ok(StepOutcome::InProgress);
        };
        let Some(request) = request else {
            return Ok(StepOutcome::Complete(Ok(Flow::new(None, Ok(None)))));
        };
        let current = dma.command_request(request.channel());
        if current != Ok(Some(request)) {
            return Ok(StepOutcome::Complete(Ok(Flow::new(
                None,
                current.map(|_| None),
            ))));
        }
        let work = match request {
            CommandRequest::Fetch(snapshot) => snapshot.work,
            CommandRequest::Resume(task) => task.work,
        };
        let Some(channel) = dma.channels.get_mut(work.channel.index()) else {
            return Ok(StepOutcome::Complete(Ok(Flow::new(
                None,
                Err(Error::Unmapped),
            ))));
        };
        if channel.resident.is_some() {
            return Ok(StepOutcome::Complete(Ok(Flow::new(None, Ok(None)))));
        }
        channel.resident = Some(work);
        if matches!(request, CommandRequest::Resume(_)) {
            dma.active = Some(work);
        }
        let value = match request {
            CommandRequest::Fetch(snapshot) => dma
                .execution_context()
                .select(work)
                .and_then(|mut selected| fetch(&mut selected, snapshot)),
            CommandRequest::Resume(_) => channel
                .accepted_command()
                .copied()
                .ok_or(Error::Reset)
                .map(|command| {
                    Some(FetchedCommand {
                        work,
                        command,
                        admission: CommandAdmission::Resume,
                    })
                }),
        };
        Ok(StepOutcome::Complete(Ok(Flow::new(Some(work), value))))
    }
}

fn fetch<M: MemoryAccess<SB>, SB>(
    context: &mut SelectedResources<'_, M, SB>,
    snapshot: CommandSnapshot,
) -> Result<Option<FetchedCommand>, Error> {
    if snapshot.work.channel != context.id {
        return Err(Error::Routing);
    }
    let work = snapshot.work;
    let channel = context.channel(work)?;
    if !channel.enabled()
        || channel.work.is_some()
        || channel.command.pointers().consumer != snapshot.consumer
        || channel.command.entry(snapshot.consumer).ok() != Some(snapshot.address)
        || channel.command.pointers().is_empty()
        || context.active.is_some()
    {
        return Ok(None);
    }
    let memory = BoundMemory::new(
        context.memory,
        (context.sideband)(DmaAccess {
            channel: work.channel,
            context: ContextId::new(context.selected.registers.values.config.ring_context.raw()),
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
                    return Err(Error::ChannelFatal(fatal));
                }
                context.changed();
                return Ok(None);
            }
            if let Err(fatal) = context.selected.command.consume() {
                context.selected.latch(fatal);
                context.changed();
                return Err(Error::ChannelFatal(fatal));
            }
            context.selected.work = Some(Accepted {
                command,
                unread: None,
                payload: None,
                progress: None,
                fill_completed: 0,
                timestamp: None,
            });
            *context.active = Some(work);
            context.generation = work.generation;
            context.changed();
            Ok(Some(FetchedCommand {
                work,
                command,
                admission: CommandAdmission::Fresh,
            }))
        }
        Ok(None) => Ok(None),
        Err(fatal) => {
            context.selected.latch(fatal);
            context.changed();
            Err(Error::ChannelFatal(fatal))
        }
    }
}
