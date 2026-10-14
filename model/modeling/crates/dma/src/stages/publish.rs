use crate::channel::Action;
use crate::{Dma, Error, Work};
use hardware_interface_pipeline::{RunError, Stage, StepOutcome, StepResult};
use hardware_interface_runtime::MemoryAccess;
use pdma_model_contract::dma::ChannelId;

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

impl<M: MemoryAccess<SB>, SB> Stage<Dma<M, SB>> for Publish {
    type Input = Work;
    type Output = ();
    type Error = Error;
    type State = Option<Work>;
    fn start(input: Self::Input) -> Self::State {
        Some(input)
    }

    fn step(&mut self, device: &mut Dma<M, SB>, state: &mut Self::State) -> StepResult<(), Error> {
        let work = *state.as_ref().ok_or(RunError::Exhausted)?;
        if work.channel() != self.channel {
            *state = None;
            return Ok(StepOutcome::Complete(Err(Error::Routing)));
        }
        match device.work_ready(work) {
            Err(error) => {
                *state = None;
                return Ok(StepOutcome::Complete(Err(error)));
            }
            Ok(false) => return Ok(StepOutcome::InProgress),
            Ok(true) => {}
        }
        let context = match device.channel_context(work.channel()) {
            Ok(context) => context,
            Err(error) => {
                *state = None;
                return Ok(StepOutcome::Complete(Err(error)));
            }
        };
        let Some(Action::Respond(response)) =
            context.channel.work.as_ref().map(|work| &work.action)
        else {
            *state = None;
            return Ok(StepOutcome::Complete(Err(Error::Routing)));
        };
        let response = *response;
        let was_empty = context.channel.response.pointers().is_empty();
        match context.channel.response.push(&context.memory, &response) {
            Ok(true) => {
                context.channel.published(&response, was_empty);
                context.channel.work = None;
                device.active = None;
                *state = None;
                Ok(StepOutcome::Complete(Ok(())))
            }
            Ok(false) => Ok(StepOutcome::InProgress),
            Err(fatal) => {
                context.channel.latch(fatal);
                *state = None;
                Ok(StepOutcome::Complete(Err(Error::ChannelFatal(fatal))))
            }
        }
    }
}
