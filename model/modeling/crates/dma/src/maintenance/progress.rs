use super::MaintenanceError;
use crate::{Channel, Dma, Maintenance, Work, references};
use pdma_model_contract::dma::ChannelId;

impl Maintenance {
    pub(crate) fn observe(
        &mut self,
        id: ChannelId,
        channel: &Channel,
        active: Option<Work>,
        halted: bool,
    ) {
        if self.result != Ok(false) {
            return;
        }
        for frontier in self
            .frontiers
            .iter_mut()
            .filter(|frontier| frontier.channel == id)
        {
            let active = active.is_some_and(|work| work.channel == id);
            let target = channel
                .accepted_command()
                .is_some_and(|task| references(task).contains(&Some(self.context)));
            let blocked = channel.accepted_command().is_some()
                && !active
                && channel.registers.values.fault_status.faulted()
                && (!frontier.passed || target)
                && !(self.abort && target);
            frontier.failed |= halted
                || channel.halted()
                || channel.resetting()
                || channel.generation != frontier.generation
                || blocked
                || ((!frontier.passed || active) && !channel.enabled());
            frontier.passed |= channel
                .command
                .pointers()
                .consumer
                .wrapping_sub(frontier.consumer)
                >= frontier.pending;
            frontier.referenced = target;
        }
    }

    pub(crate) fn finish_observation(&mut self) {
        if self.result != Ok(false) {
            return;
        }
        if let Some(frontier) = self.frontiers.iter().find(|frontier| frontier.failed) {
            self.result = Err(MaintenanceError::ChannelStopped(frontier.channel));
        } else if self
            .frontiers
            .iter()
            .all(|frontier| frontier.passed && !frontier.referenced)
        {
            self.result = Ok(true);
        }
    }
}

impl<M, S> Dma<M, S> {
    pub(crate) fn observe_channel(&mut self, id: ChannelId) {
        let halted = self.halted() || self.resetting();
        if let (Some(maintenance), Some(channel)) =
            (&mut self.maintenance, self.channels.get(id.index()))
        {
            maintenance.observe(id, channel, self.active, halted);
            maintenance.finish_observation();
        }
    }

    pub(crate) fn refresh_maintenance(&mut self) {
        let halted = self.halted() || self.resetting();
        if let Some(maintenance) = &mut self.maintenance {
            for (index, channel) in self.channels.iter().enumerate() {
                maintenance.observe(ChannelId::new(index), channel, self.active, halted);
            }
            maintenance.finish_observation();
        }
    }
}
