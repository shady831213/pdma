use crate::{ChannelTask, Dma, Error, Fetch};
use hardware_interface_pipeline::{RunError, Stage, StepOutcome, StepResult};
use hardware_interface_runtime::MemoryAccess;
use pdma_model_contract::dma::{ChannelId, Command};

pub(super) struct Frame<'a, T, M, SB> {
    pub device: &'a mut Dma<M, SB>,
    pub saved: &'a mut [Option<T>],
}

/// Produces a channel candidate without accepting a descriptor.
#[derive(Debug)]
pub struct Candidate(ChannelId);

impl Candidate {
    /// Binds a candidate source to one physical channel.
    pub const fn new(channel: ChannelId) -> Self {
        Self(channel)
    }
}

impl<M, SB> Stage<Dma<M, SB>> for Candidate {
    type Input = ();
    type Output = Option<ChannelId>;
    type Error = Error;
    type State = ();

    fn start((): ()) {}

    fn step(&mut self, device: &mut Dma<M, SB>, (): &mut ()) -> StepResult<Self::Output, Error> {
        Ok(StepOutcome::Complete(Ok(Fetch::new(self.0)
            .eligible(device)
            .then_some(self.0))))
    }
}

impl<T, M, SB> Stage<Frame<'_, T, M, SB>> for Candidate {
    type Input = ();
    type Output = Option<ChannelId>;
    type Error = Error;
    type State = ();

    fn start((): ()) {}

    fn step(
        &mut self,
        frame: &mut Frame<'_, T, M, SB>,
        state: &mut (),
    ) -> StepResult<Self::Output, Error> {
        self.step(frame.device, state)
    }
}

#[derive(Debug)]
pub(super) struct Accept;

impl<M: MemoryAccess<SB>, SB> Stage<Dma<M, SB>> for Accept {
    type Input = ChannelId;
    type Output = Option<ChannelTask<Command>>;
    type Error = Error;
    type State = Option<ChannelId>;

    fn start(channel: ChannelId) -> Self::State {
        Some(channel)
    }

    fn step(
        &mut self,
        device: &mut Dma<M, SB>,
        state: &mut Self::State,
    ) -> StepResult<Self::Output, Error> {
        let channel = state.take().ok_or(RunError::Exhausted)?;
        Ok(match Fetch::new(channel).step(device, &mut ())? {
            StepOutcome::Complete(result) => StepOutcome::Complete(result.map(Some)),
            StepOutcome::InProgress => StepOutcome::Complete(Ok(None)),
        })
    }
}
