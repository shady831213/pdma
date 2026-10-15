use crate::{CommandSnapshot, Dma, Error, Work};
use hardware_interface_pipeline::{Stage, StepOutcome, StepResult};
use pdma_model_contract::dma::ChannelId;
use pdma_model_contract::ring::Ring;

/// Non-owning request for fetching or continuing shared command execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandRequest {
    /// Fetch the channel's current command when the request is accepted.
    Fetch(CommandSnapshot),
    /// Resume the channel's already accepted, parked command.
    Resume(Work),
}

impl CommandRequest {
    /// Identifies the requesting channel without transferring task ownership.
    pub const fn channel(self) -> ChannelId {
        match self {
            Self::Fetch(snapshot) => snapshot.work.channel,
            Self::Resume(work) => work.channel,
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
}

impl<M, SB> Stage<Dma<M, SB>> for Candidate {
    type Input = ();
    type Output = Option<CommandRequest>;
    type Error = Error;
    type State = ();

    fn start((): ()) {}

    fn step(&mut self, device: &mut Dma<M, SB>, (): &mut ()) -> StepResult<Self::Output, Error> {
        Ok(StepOutcome::Complete(device.command_request(self.0)))
    }
}

impl<M, SB> Dma<M, SB> {
    /// Snapshots an eligible channel without accepting its descriptor.
    ///
    /// # Errors
    /// Returns `Error::Unmapped` for an absent channel.
    pub fn command_request(&self, id: ChannelId) -> Result<Option<CommandRequest>, Error> {
        let channel = self.channels.get(id.index()).ok_or(Error::Unmapped)?;
        if self.active.is_some() {
            return Ok(None);
        }
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
        let work = Work {
            channel: id,
            generation: channel.generation,
        };
        Ok(if channel.work.is_some() {
            Some(CommandRequest::Resume(work))
        } else if channel.command.pointers().is_empty() {
            None
        } else {
            Some(CommandRequest::Fetch(CommandSnapshot {
                work,
                consumer: channel.command.pointers().consumer,
                address: channel
                    .command
                    .entry(channel.command.pointers().consumer)
                    .map_err(|_| Error::Cursor)?,
            }))
        })
    }
}
