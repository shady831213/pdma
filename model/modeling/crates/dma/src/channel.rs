mod command;
mod mmio;
mod response;
#[cfg(test)]
mod tests;

pub use command::CommandRing;
pub use response::ResponseRing;

use crate::{ChannelFatal, ChannelTask};
use hardware_interface_schema::RegisterWindow;
use pdma_model_contract::dma::{
    ChannelAdmin, ChannelBlock, ChannelFatalCode, ChannelIrqCauseValues, ChannelRegisters,
    ContextId, FaultStatusValues, ResetValues, Response,
};

/// One channel owns two rings, sticky fatal state and interrupt controls.
#[derive(Debug)]
pub struct Channel {
    /// Host-produced command ring.
    pub command: CommandRing,
    /// Dma-produced response ring.
    pub response: ResponseRing,
    pub(crate) registers: RegisterWindow<ChannelRegisters>,
    pub(crate) admin: RegisterWindow<ChannelAdmin>,
    pub(crate) configured: bool,
    pub(crate) irq: bool,
    pub(crate) generation: u64,
    pub(crate) parked: Option<ChannelTask<[Option<ContextId>; 3]>>,
}

impl Default for Channel {
    fn default() -> Self {
        Self {
            command: CommandRing::new(ChannelBlock::COMMAND.offset()),
            response: ResponseRing::new(ChannelBlock::RESPONSE.offset()),
            registers: RegisterWindow::new(ChannelBlock::CONTROL.offset()),
            admin: RegisterWindow::new(0),
            configured: false,
            irq: false,
            generation: 0,
            parked: None,
        }
    }
}

impl Channel {
    pub(crate) const fn suspend(&mut self) {
        self.registers.values.fault_status = FaultStatusValues::new(1);
    }

    pub(crate) fn latch(&mut self, fatal: ChannelFatal) {
        if self.registers.values.fatal.code() == Ok(ChannelFatalCode::None) {
            self.registers.values.fatal = self.registers.values.fatal.with_code(fatal.code());
            self.irq |= self.registers.values.irq_enable.raw() != 0;
        }
    }

    pub(crate) fn update_irq(&mut self) {
        self.registers.values.irq_cause = ChannelIrqCauseValues::new(
            u32::from(!self.response.pointers().is_empty())
                | (u32::from(self.response.last_ioc.is_some()) << 1)
                | (self.registers.values.irq_cause.raw() & 4)
                | (u32::from(self.registers.values.fatal.code() != Ok(ChannelFatalCode::None))
                    << 3),
        );
    }

    pub(crate) fn published(&mut self, response: &Response, was_empty: bool) {
        let error = response.status.halts();
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
            self.admin.values.reset = ResetValues::new(1);
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
        self.admin.values.reset.request()
    }

    pub(crate) fn halted(&self) -> bool {
        match self.registers.values.fatal.code() {
            Ok(ChannelFatalCode::None) => self.registers.values.irq_cause.response_error(),
            Ok(ChannelFatalCode::CommandRead | ChannelFatalCode::ResponseWrite) | Err(_) => true,
        }
    }
    pub(crate) const fn enabled(&self) -> bool {
        self.registers.values.enable.raw() != 0
    }
}
