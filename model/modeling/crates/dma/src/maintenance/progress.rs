use super::MaintenanceError;
use crate::{Dma, references};
use hardware_interface_runtime::MemoryAccess;

impl<M, S> Dma<M, S> {
    pub(crate) fn refresh_maintenance(&mut self) {
        let halted = self.halted() || self.resetting();
        let Some(maintenance) = &mut self.maintenance else {
            return;
        };
        if maintenance.result != Ok(false) {
            return;
        }
        let mut failure = None;
        for frontier in &mut maintenance.frontiers {
            let Some(channel) = self.channels.get(frontier.channel.index()) else {
                frontier.failed = true;
                failure.get_or_insert(MaintenanceError::InvalidChannel(frontier.channel));
                continue;
            };
            let active = self
                .active
                .as_ref()
                .is_some_and(|active| active.channel() == frontier.channel);
            if halted
                || channel.halted()
                || channel.resetting()
                || channel.generation != frontier.generation
                || ((!frontier.passed || active) && !channel.enabled())
            {
                frontier.failed = true;
                failure.get_or_insert(MaintenanceError::ChannelStopped(frontier.channel));
            }
            frontier.passed |= channel
                .command
                .pointers()
                .consumer
                .wrapping_sub(frontier.consumer)
                >= frontier.pending;
        }
        if let Some(failure) = failure {
            maintenance.result = Err(failure);
        } else if maintenance.frontiers.iter().all(|f| f.passed)
            && !self
                .active
                .as_ref()
                .is_some_and(|active| active.value.contains(&Some(maintenance.context)))
        {
            maintenance.result = Ok(true);
        }
    }
}

impl<M: MemoryAccess<S>, S> Dma<M, S> {
    pub(crate) fn discard_aborted(&mut self) -> bool {
        self.refresh_maintenance();
        let Some(maintenance) = &self.maintenance else {
            return false;
        };
        if maintenance.result != Ok(false) || !maintenance.abort || self.active.is_some() {
            return false;
        }
        let context = maintenance.context;
        for index in 0..maintenance.frontiers.len() {
            let Some(frontier) = self
                .maintenance
                .as_ref()
                .and_then(|m| m.frontiers.get(index))
            else {
                return false;
            };
            if frontier.passed {
                continue;
            }
            let id = frontier.channel;
            let Ok(selected) = self.channel_context(id) else {
                return false;
            };
            match selected.channel.command.peek(&selected.memory) {
                Ok(Some(command)) if references(&command).contains(&Some(context)) => {
                    if let Err(fault) = selected.channel.command.consume() {
                        selected.channel.latch(fault);
                    }
                    self.refresh_maintenance();
                    return true;
                }
                Ok(Some(_) | None) => {}
                Err(fault) => {
                    selected.channel.latch(fault);
                    selected.channel.update_irq();
                }
            }
        }
        self.refresh_maintenance();
        false
    }
}
