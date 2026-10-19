use crate::SelectedResources;
use crate::channel::Accepted;
use crate::{CommandRequest, CommandSnapshot, Dma, Error, FetchedCommand, Flow};
use hardware_interface_pipeline::{Stage, StepOutcome, StepResult};
use hardware_interface_runtime::{BoundMemory, MemoryAccess};
use pdma_model_contract::dma::{ContextId, DmaAccess};
use pdma_model_contract::ring::Ring;

/// Accepts a fresh task request and transfers its identity into execution dataflow.
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
        let (work, snapshot) = match request {
            CommandRequest::Fetch(snapshot) => (snapshot.work, Some(snapshot)),
            CommandRequest::Resume(task) => (task.work, None),
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
        let value = snapshot.map_or(Ok(None), |snapshot| {
            fetch(&mut dma.execution_context().select(work)?, snapshot)
        });
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
    let aborting = context.abort_context.is_some();
    let channel = context.channel(work)?;
    if !channel.enabled()
        || channel.work.is_some()
        || channel.command.pointers().consumer != snapshot.consumer
        || channel.command.entry(snapshot.consumer).ok() != Some(snapshot.address)
        || channel.command.pointers().is_empty()
        || (!aborting && channel.response.pointers().pending() >= channel.response.capacity())
        || context.active.is_some()
    {
        return Ok(None);
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
                    return Err(Error::ChannelFatal(fatal));
                }
                context.changed();
                return Ok(None);
            }
            if context.selected.response.pointers().pending()
                >= context.selected.response.capacity()
            {
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
                completion: None,
                progress: None,
            });
            *context.active = Some(work);
            context.generation = work.generation;
            context.changed();
            Ok(Some(FetchedCommand { work, command }))
        }
        Ok(None) => Ok(None),
        Err(fatal) => {
            context.selected.latch(fatal);
            context.changed();
            Err(Error::ChannelFatal(fatal))
        }
    }
}
