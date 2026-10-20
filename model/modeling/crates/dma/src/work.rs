use crate::Dma;
use pdma_model_contract::dma::ChannelId;

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

impl<M, S> Dma<M, S> {
    pub(crate) fn release_residency(&mut self, work: Work) {
        let halted = self.halted();
        let abort_context = self
            .maintenance
            .as_ref()
            .filter(|maintenance| {
                maintenance.abort
                    && maintenance.result == Ok(false)
                    && maintenance
                        .frontiers
                        .iter()
                        .any(|frontier| frontier.channel == work.channel)
            })
            .map(|maintenance| maintenance.context);
        if let Some(channel) = self.channels.get_mut(work.channel.index()) {
            if channel.resident != Some(work) {
                return;
            }
            channel.resident = None;
            if halted
                || channel.halted()
                || ((self.active != Some(work)
                    || channel.work.as_ref().is_some_and(|work| work.waiting))
                    && abort_context.is_some_and(|context| {
                        channel.accepted_command().is_some_and(|command| {
                            crate::references(command).contains(&Some(context))
                        })
                    }))
            {
                channel.retire();
            }
            if channel.resetting() {
                channel.finish_reset();
            }
        }
        self.release_invalid_grant();
        self.finish_device_reset();
        self.observe_channel(work.channel);
    }

    pub(crate) fn release_invalid_grant(&mut self) {
        if self.active.is_some_and(|work| {
            self.channels
                .get(work.channel.index())
                .is_none_or(|channel| {
                    channel.generation != work.generation || channel.work.is_none()
                })
        }) {
            self.active = None;
        }
    }
}
