use crate::{Completion, Error, ExecutionContext};
use hardware_interface_pipeline::{RunError, Stage, StepOutcome, StepResult};
use hardware_interface_runtime::{BoundMemory, MemoryAccess};
use pdma_model_contract::dma::{ChannelId, ContextId, DmaAccess};

/// Publishes the selected channel's retained response before retiring its work.
#[derive(Debug)]
pub struct Publish {
    channel: ChannelId,
}
impl Publish {
    /// Binds publication to the specified response-ring owner.
    pub const fn new(channel: ChannelId) -> Self {
        Self { channel }
    }
}
impl<M: MemoryAccess<SB>, SB> Stage<ExecutionContext<'_, M, SB>> for Publish {
    type Input = Completion;
    type Output = ();
    type Error = crate::Error;
    type State = Option<Completion>;
    fn start(input: Self::Input) -> Self::State {
        Some(input)
    }
    fn step(
        &mut self,
        context: &mut ExecutionContext<'_, M, SB>,
        state: &mut Self::State,
    ) -> StepResult<(), crate::Error> {
        let completion = state.as_ref().ok_or(RunError::Exhausted)?;
        let work = completion.work;
        if work.channel() != self.channel {
            *state = None;
            return Ok(StepOutcome::Complete(Err(Error::Routing)));
        }
        match context.ready(work) {
            Err(error) => {
                *state = None;
                return Ok(StepOutcome::Complete(Err(error)));
            }
            Ok(false) => return Ok(StepOutcome::InProgress),
            Ok(true) => {}
        }
        let memory = BoundMemory::new(
            context.memory,
            (context.sideband)(DmaAccess {
                channel: work.channel,
                context: ContextId::new(context.selected.registers.values.ring_context.raw()),
            }),
        );
        let was_empty = context.selected.response.pointers().is_empty();
        match context
            .selected
            .response
            .push(&memory, &completion.response)
        {
            Ok(true) => {
                context.selected.published(&completion.response, was_empty);
                context.selected.work = None;
                if *context.active == Some(work) {
                    *context.active = None;
                }
                context.changed();
                *state = None;
                Ok(StepOutcome::Complete(Ok(())))
            }
            Ok(false) => Ok(StepOutcome::InProgress),
            Err(fatal) => {
                context.selected.latch(fatal);
                context.changed();
                *state = None;
                Ok(StepOutcome::Complete(Err(Error::ChannelFatal(fatal))))
            }
        }
    }
}
