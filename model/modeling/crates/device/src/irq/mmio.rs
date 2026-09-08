use super::source::IrqSourceState;
use crate::queue::exact::PdmaControlError;
use hardware_interface_schema::RegisterLayout;
use hardware_interface_schema::register::RegisterSpec;
use pdma_model_contract::abi::registers::{
    IrqSourceRegisters, IrqSourceRegistersRegisterId, IrqStateBit,
};

impl IrqSourceState {
    pub(crate) fn read_register(&self, offset: usize, records_pending: bool) -> Option<u64> {
        if offset == IrqSourceRegisters::STATUS.offset() {
            return Some(u64::from(self.status(records_pending)));
        }
        if offset == IrqSourceRegisters::PENDING_MESSAGES.offset() {
            return Some(u64::try_from(self.pending_messages).unwrap_or(u64::MAX));
        }
        IrqSourceRegisters::read_register(&self.registers, offset)
    }

    pub(crate) fn write_register(
        &mut self,
        register: RegisterSpec<IrqSourceRegistersRegisterId>,
        value: u64,
        records_pending: bool,
    ) -> Result<bool, PdmaControlError> {
        use IrqSourceRegistersRegisterId as Id;
        match register.id() {
            Id::RouteKind => {
                if self.is_active() || value > 1 {
                    return Err(PdmaControlError::InvalidTransaction);
                }
            }
            Id::RouteVector => {
                if self.is_active() || usize::try_from(value).is_err() {
                    return Err(PdmaControlError::InvalidTransaction);
                }
            }
            Id::Enabled | Id::Masked => {
                if value > 1 {
                    return Err(PdmaControlError::InvalidTransaction);
                }
            }
            Id::Ack => {
                if value != 1 {
                    return Err(PdmaControlError::InvalidTransaction);
                }
                let before = self.cause_pending;
                self.ack(records_pending);
                return Ok(before != self.cause_pending);
            }
            Id::Status | Id::PendingMessages => {
                return Err(PdmaControlError::InvalidTransaction);
            }
        }
        let previous = IrqSourceRegisters::read_register(&self.registers, register.offset());
        IrqSourceRegisters::write_register(&mut self.registers, register.offset(), value)
            .ok_or(PdmaControlError::InvalidTransaction)?;
        Ok(previous != Some(value))
    }

    fn status(&self, records_pending: bool) -> u32 {
        let mut status = IrqStateBit::Present.mask();
        for (set, bit) in [
            (self.enabled(), IrqStateBit::Enabled),
            (self.masked(), IrqStateBit::Masked),
            (self.cause_pending, IrqStateBit::CausePending),
            (
                self.asserts_line() || self.can_deliver_message(),
                IrqStateBit::Deliverable,
            ),
            (self.asserts_line(), IrqStateBit::LineAsserted),
            (records_pending, IrqStateBit::RecordsPending),
        ] {
            if set {
                status |= bit.mask();
            }
        }
        status
    }
}
