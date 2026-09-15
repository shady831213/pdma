mod command;
mod mmio;
mod response;
#[cfg(test)]
mod tests;

pub use command::CommandRing;
pub use response::ResponseRing;

use crate::Fault;
use hardware_interface_schema::RegisterWindow;
use pdma_model_contract::{
    ChannelBlock, ChannelFaultCode, ChannelIrqCauseValues, ChannelRegisters, ResetValues, Response,
    ResponseStatus,
};

/// One channel owns two rings, sticky fault state and interrupt controls.
#[derive(Debug)]
pub struct Channel {
    /// Host-produced command ring.
    pub command: CommandRing,
    /// Device-produced response ring.
    pub response: ResponseRing,
    pub(crate) registers: RegisterWindow<ChannelRegisters>,
    pub(crate) configured: bool,
    pub(crate) irq: bool,
    pub(crate) generation: u64,
}

impl Default for Channel {
    fn default() -> Self {
        Self {
            command: CommandRing::new(ChannelBlock::COMMAND.offset()),
            response: ResponseRing::new(ChannelBlock::RESPONSE.offset()),
            registers: RegisterWindow::new(ChannelBlock::CONTROL.offset()),
            configured: false,
            irq: false,
            generation: 0,
        }
    }
}

impl Channel {
    pub(crate) fn latch(&mut self, fault: Fault) {
        if self.registers.values.fault.code() == Ok(ChannelFaultCode::None) {
            self.registers.values.fault = self.registers.values.fault.with_code(fault.code());
            self.irq |= self.registers.values.irq_enable.raw() != 0;
        }
    }

    pub(crate) fn update_irq(&mut self) {
        self.registers.values.irq_cause = ChannelIrqCauseValues::new(
            u32::from(!self.response.pointers().is_empty())
                | (u32::from(self.response.last_ioc.is_some()) << 1)
                | (self.registers.values.irq_cause.raw() & 4)
                | (u32::from(self.registers.values.fault.code() != Ok(ChannelFaultCode::None))
                    << 3),
        );
    }

    pub(crate) fn published(&mut self, response: &Response, was_empty: bool) {
        let error = response.status != ResponseStatus::Success;
        if error {
            self.registers.values.irq_cause =
                ChannelIrqCauseValues::new(self.registers.values.irq_cause.raw() | 4);
        }
        let mask = self.registers.values.irq_mask;
        self.irq |= self.registers.values.irq_enable.raw() != 0
            && ((was_empty && !mask.response_non_empty())
                || (response.flags.ioc && !mask.ioc())
                || error);
    }

    pub(crate) fn request_reset(&mut self) {
        if !self.resetting() {
            self.generation = self.generation.wrapping_add(1);
            self.registers.values.reset = ResetValues::new(1);
        }
    }

    pub(crate) fn finish_reset(&mut self) {
        let generation = self.generation;
        *self = Self {
            generation,
            ..Self::default()
        };
    }

    pub(crate) fn resetting(&self) -> bool {
        self.registers.values.reset.request()
    }

    pub(crate) fn halted(&self) -> bool {
        match self.registers.values.fault.code() {
            Ok(ChannelFaultCode::None) => self.registers.values.irq_cause.response_error(),
            Ok(ChannelFaultCode::CommandRead | ChannelFaultCode::ResponseWrite) | Err(_) => true,
        }
    }
    pub(crate) const fn enabled(&self) -> bool {
        self.registers.values.enable.raw() != 0
    }
}
