use crate::{Dma, Error};
use hardware_interface_pipeline::{Stage, StepOutcome, StepResult};
use pdma_model_contract::dma::ChannelId;
use pdma_model_contract::ring::Ring;

/// Non-owning request for shared execution; no descriptor has been accepted yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandRequest {
    /// Fetch the channel's current command when the request is accepted.
    Fetch(ChannelId),
    /// Resume the channel's already accepted, parked command.
    Resume(ChannelId),
}

impl CommandRequest {
    /// Identifies the requesting channel without transferring task ownership.
    pub const fn channel(self) -> ChannelId {
        match self {
            Self::Fetch(channel) | Self::Resume(channel) => channel,
        }
    }
}

/// Produces a channel candidate without accepting a descriptor.
#[derive(Debug)]
pub struct Candidate(ChannelId);

impl Candidate {
    /// Binds a candidate source to one physical channel.
    pub const fn new(channel: ChannelId) -> Self {
        Self(channel)
    }

    pub(crate) fn request<M, SB>(
        &self,
        device: &Dma<M, SB>,
    ) -> Result<Option<CommandRequest>, Error> {
        device.command_request(self.0)
    }
}

impl<M, SB> Stage<Dma<M, SB>> for Candidate {
    type Input = ();
    type Output = Option<CommandRequest>;
    type Error = Error;
    type State = ();

    fn start((): ()) {}

    fn step(&mut self, device: &mut Dma<M, SB>, (): &mut ()) -> StepResult<Self::Output, Error> {
        Ok(StepOutcome::Complete(self.request(device)))
    }
}

impl<M, SB> Dma<M, SB> {
    pub(crate) fn command_request(&self, id: ChannelId) -> Result<Option<CommandRequest>, Error> {
        let channel = self.channels.get(id.index()).ok_or(Error::Unmapped)?;
        if self.halted()
            || self.resetting()
            || !channel.enabled()
            || channel.resetting()
            || channel.halted()
            || channel.registers.values.fault_status.faulted()
            || channel.response.pointers().pending() >= channel.response.capacity()
        {
            return Ok(None);
        }
        Ok(if channel.parked.is_some() {
            Some(CommandRequest::Resume(id))
        } else if channel.command.pointers().is_empty() {
            None
        } else {
            Some(CommandRequest::Fetch(id))
        })
    }
}
