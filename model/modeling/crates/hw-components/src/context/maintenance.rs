use super::ContextTable;
use crate::windows::ChannelWindows;
use hardware_interface_schema::{RegisterAccessError, RegisterWindow};
use pdma_model_contract::dma::{ChannelId, ContextId};
use pdma_model_contract::hw_components::context::{
    ContextManagementRegisters, ContextRequestAction, ContextRequestValues, ContextResultState,
    ContextResultValues,
};
use pdma_model_contract::register::ReadWordValues;
use pdma_model_dma::{Dma, MaintenanceError};

impl ContextTable {
    pub(super) fn write_management<M, S>(
        &mut self,
        (address, bytes): (usize, &[u8]),
        hardware: (&mut Dma<M, S>, &ChannelWindows),
    ) -> Result<(), RegisterAccessError> {
        let mut next = RegisterWindow::<ContextManagementRegisters>::new(self.management.base());
        next.values = self.management.values;
        next.write(address, bytes)?;
        if self.pending.is_some() {
            return Err(RegisterAccessError::WriteForbidden {
                offset: address - self.management.base(),
            });
        }
        let action =
            next.values
                .request
                .action()
                .map_err(|_| RegisterAccessError::WriteForbidden {
                    offset: ContextManagementRegisters::REQUEST.offset(),
                })?;
        let context = ContextId::new(next.values.request.context_id());
        self.management.values = next.values;
        self.management.values.request = ContextRequestValues::RESET;
        self.start((action, context), hardware);
        Ok(())
    }

    fn start<M, S>(
        &mut self,
        (action, context): (ContextRequestAction, ContextId),
        (dma, windows): (&mut Dma<M, S>, &ChannelWindows),
    ) {
        let abort = match action {
            ContextRequestAction::Idle => return,
            ContextRequestAction::Drain => false,
            ContextRequestAction::Abort => true,
        };
        for word in &mut self.failed {
            word.values.bits = ReadWordValues::RESET;
        }
        let Some(entry) = usize::try_from(context.raw())
            .ok()
            .and_then(|index| self.entries.get(index))
            .filter(|entry| entry.values.valid.enabled())
        else {
            self.result(ContextResultState::InvalidContext);
            return;
        };
        let owner = pdma_model_contract::memory::FunctionId::new(entry.values.owner.raw());
        let mut channels = Vec::new();
        if channels.try_reserve_exact(windows.channel_count()).is_err() {
            self.result(ContextResultState::Failed);
            return;
        }
        for index in 0..windows.channel_count() {
            let channel = ChannelId::new(index);
            if windows.owner(channel) == Some(owner) {
                channels.push(channel);
            }
        }
        let result = if abort {
            dma.abort_context(context, &channels)
        } else {
            dma.drain_context(context, &channels)
        };
        match result {
            Ok(()) => {
                self.pending = Some(context);
                self.result(ContextResultState::Pending);
                self.step(dma);
            }
            Err(error) => self.failed(error),
        }
    }

    #[doc = "Publishes completion without clearing entry validity; firmware owns invalidation."]
    pub fn step<M, S>(&mut self, dma: &Dma<M, S>) {
        let Some(context) = self.pending else {
            return;
        };
        match dma.context_status(context) {
            Ok(false) => {}
            Ok(true) => {
                self.pending = None;
                self.result(ContextResultState::Success);
            }
            Err(error) => {
                for channel in dma.failed_context_channels(context) {
                    self.mark_failed(channel);
                }
                self.pending = None;
                self.failed(error);
            }
        }
    }

    fn mark_failed(&mut self, channel: ChannelId) {
        if let Some(word) = self.failed.get_mut(channel.index() / 32) {
            word.values.bits =
                ReadWordValues::new(word.values.bits.raw() | (1 << (channel.index() % 32)));
        }
    }

    fn failed(&mut self, error: MaintenanceError) {
        let state = match error {
            MaintenanceError::Busy | MaintenanceError::RingBound(_) => ContextResultState::Busy,
            MaintenanceError::ChannelStopped(channel)
            | MaintenanceError::InvalidChannel(channel) => {
                self.mark_failed(channel);
                ContextResultState::Failed
            }
            MaintenanceError::Unknown | MaintenanceError::Allocation => ContextResultState::Failed,
        };
        self.result(state);
    }

    const fn result(&mut self, state: ContextResultState) {
        self.management.values.result = ContextResultValues::RESET.with_state(state);
    }
}
