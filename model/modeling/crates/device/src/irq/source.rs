use pdma_model_contract::abi::registers::IrqSourceRegistersValues;
#[cfg(test)]
mod tests;
use pdma_model_contract::semantic::{
    DeviceEpoch, EpochIrqSource, IrqLine, IrqMessage, IrqMessageId, IrqRoute, IrqSource, IrqVector,
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct IrqSourceState {
    pub(super) registers: IrqSourceRegistersValues,
    pub(super) cause_pending: bool,
    pub(super) pending_messages: usize,
    next_message_id: usize,
}

impl IrqSourceState {
    pub(crate) fn route(&self) -> IrqRoute {
        let vector = IrqVector::new(
            usize::try_from(self.registers.route_vector.raw()).unwrap_or(usize::MAX),
        );
        if self.registers.route_kind.raw() == 0 {
            IrqRoute::Line(IrqLine::new(vector))
        } else {
            IrqRoute::Message(vector)
        }
    }

    pub(crate) fn has_line_cause(&self) -> bool {
        matches!(self.route(), IrqRoute::Line(_)) && self.cause_pending
    }

    pub(crate) fn asserts_line(&self) -> bool {
        self.has_line_cause() && self.enabled() && !self.masked()
    }

    pub(crate) fn can_deliver_message(&self) -> bool {
        matches!(self.route(), IrqRoute::Message(_))
            && self.enabled()
            && !self.masked()
            && self.pending_messages != 0
    }

    pub(crate) fn take_message(
        &mut self,
        epoch: DeviceEpoch,
        source: IrqSource,
    ) -> Option<IrqMessage> {
        if !self.can_deliver_message() {
            return None;
        }
        let vector = match self.route() {
            IrqRoute::Line(_) => return None,
            IrqRoute::Message(vector) => vector,
        };
        let message = IrqMessage::new(
            IrqMessageId::new(self.next_message_id),
            vector,
            EpochIrqSource::new(epoch, source),
        );
        self.next_message_id = self.next_message_id.saturating_add(1);
        self.pending_messages -= 1;
        Some(message)
    }

    pub(crate) fn latch(&mut self) -> bool {
        if !self.enabled() {
            return false;
        }
        self.cause_pending = true;
        match self.route() {
            IrqRoute::Line(_) => {}
            IrqRoute::Message(_) => {
                self.pending_messages = self.pending_messages.saturating_add(1);
            }
        }
        true
    }

    pub(crate) fn ack(&mut self, records_pending: bool) {
        self.cause_pending =
            self.cause_pending && records_pending && matches!(self.route(), IrqRoute::Line(_));
    }

    pub(super) const fn enabled(&self) -> bool {
        self.registers.enabled.raw() != 0
    }

    pub(super) const fn masked(&self) -> bool {
        self.registers.masked.raw() != 0
    }

    pub(super) const fn is_active(&self) -> bool {
        self.enabled() || self.cause_pending || self.pending_messages != 0
    }
}
