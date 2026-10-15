use crate::Dma;
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
