use crate::channel::Action;
use crate::{Channel, Dma, Error};
use pdma_model_contract::dma::{ChannelId, FaultStatusValues};

/// Non-owning channel identity and reset generation for one hardware work snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Work {
    pub(crate) channel: ChannelId,
    pub(crate) generation: u64,
}

impl Work {
    /// Channel whose accepted hardware state the snapshot addresses.
    pub const fn channel(self) -> ChannelId {
        self.channel
    }
}

impl<M, SB> Dma<M, SB> {
    pub(crate) fn work_ready(&mut self, work: Work) -> Result<bool, Error> {
        let channel = self.work_channel(work)?;
        Ok(channel.enabled() && !channel.registers.values.fault_status.faulted())
    }
    pub(crate) fn work_channel(&mut self, work: Work) -> Result<&mut Channel, Error> {
        let halted = self.halted();
        let channel = self
            .channels
            .get_mut(work.channel.index())
            .ok_or(Error::Unmapped)?;
        if channel.generation != work.generation || channel.resetting() {
            return Err(Error::Reset);
        }
        if halted || channel.halted() {
            return Err(Error::Halted);
        }
        Ok(channel)
    }

    /// Borrows the active payload prefix only while transformation is requested.
    /// A resumed write returns `None`, retaining its already transformed bytes.
    ///
    /// # Errors
    /// Rejects a stale reset generation or halted channel.
    pub fn work_scratch_mut(&mut self, work: &Work) -> Result<Option<&mut [u8]>, Error> {
        let channel = self.work_channel(*work)?;
        Ok(match channel.work.as_mut().map(|work| &mut work.action) {
            Some(Action::Transform(transfer)) => transfer.bytes.get_mut(..transfer.length),
            Some(
                Action::Check(_)
                | Action::Next(_)
                | Action::Read(_)
                | Action::Write(_)
                | Action::Respond(_),
            )
            | None => None,
        })
    }
}

impl<M, SB> Dma<M, SB> {
    pub(crate) fn retire_stopped_work(&mut self) {
        let halted = self.halted();
        for (index, channel) in self.channels.iter_mut().enumerate() {
            let aborted = self.active.is_none_or(|work| work.channel.index() != index)
                && self.maintenance.as_ref().is_some_and(|maintenance| {
                    maintenance.abort
                        && maintenance.result == Ok(false)
                        && maintenance
                            .frontiers
                            .iter()
                            .any(|frontier| frontier.channel.index() == index)
                        && channel.work.as_ref().is_some_and(|work| {
                            crate::references(&work.command).contains(&Some(maintenance.context))
                        })
                });
            if channel.resetting() || channel.halted() || halted || aborted {
                channel.work = None;
                channel.registers.values.fault_status = FaultStatusValues::new(0);
                if self
                    .active
                    .is_some_and(|work| work.channel.index() == index)
                {
                    self.active = None;
                }
            }
        }
    }
}
