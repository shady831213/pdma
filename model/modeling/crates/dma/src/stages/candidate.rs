use crate::{CommandSnapshot, Dma, Error, Work};
use hw_model_pipeline::{Stage, StepOutcome, StepResult};
use pdma_model_contract::dma::ChannelId;
use pdma_model_contract::ring::Ring;

/// Non-owning identity of the accepted task at a channel's retained consumer position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RetainedTask {
    pub(crate) work: Work,
    pub(crate) consumer: u32,
}

impl RetainedTask {
    /// Channel retaining the accepted task.
    pub const fn channel(self) -> ChannelId {
        self.work.channel
    }
}

/// Non-owning request for fetching or continuing shared command execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandRequest {
    /// Fetch the channel's current command when the request is accepted.
    Fetch(CommandSnapshot),
    /// Resume the channel's already accepted, parked command.
    Resume(RetainedTask),
}

impl CommandRequest {
    /// Identifies the requesting channel without transferring task ownership.
    pub const fn channel(self) -> ChannelId {
        match self {
            Self::Fetch(snapshot) => snapshot.work.channel,
            Self::Resume(task) => task.work.channel,
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

    fn step(
        &mut self,
        device: &mut Dma<M, SB>,
        input: &mut Option<()>,
    ) -> StepResult<Self::Output, Error> {
        if input.take().is_none() {
            return Ok(StepOutcome::InProgress);
        }
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
            || channel.registers.values.recovery.fault_status.faulted()
        {
            return Ok(None);
        }
        let work = Work {
            channel: id,
            generation: channel.generation,
        };
        Ok(if channel.work.is_some() {
            Some(CommandRequest::Resume(RetainedTask {
                work,
                consumer: channel.command.pointers().consumer,
            }))
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
