mod command;
mod event;
mod mmio;
#[cfg(test)]
mod tests;
mod work;

pub use command::CommandRing;
pub use event::EventRing;

use crate::ChannelFatal;
use hardware_interface_schema::RegisterWindow;
use pdma_model_contract::dma::{
    ChannelAdmin, ChannelBlock, ChannelFatalCode, ChannelIrqCauseValues, ChannelRegisters,
    FaultStatusValues, ResetValues,
};
use pdma_model_contract::dma::{Command, EventCode};
pub use work::COPY_CHUNK_BYTES;
use work::Cursor;

/// One channel owns two rings, sticky fatal state and interrupt controls.
#[derive(Debug)]
pub struct Channel {
    /// Host-produced command ring.
    pub command: CommandRing,
    /// Dma-produced event ring.
    pub event: EventRing,
    pub(crate) registers: RegisterWindow<ChannelRegisters>,
    pub(crate) admin: RegisterWindow<ChannelAdmin>,
    pub(crate) configured: bool,
    pub(crate) irq: bool,
    pub(crate) generation: u64,
    pub(crate) work: Option<Accepted>,
    pub(crate) resident: Option<crate::Work>,
}

impl Default for Channel {
    fn default() -> Self {
        Self {
            command: CommandRing::new(ChannelBlock::COMMAND.offset()),
            event: EventRing::new(ChannelBlock::EVENT.offset()),
            registers: RegisterWindow::new(ChannelBlock::CONTROL.offset()),
            admin: RegisterWindow::new(0),
            configured: false,
            irq: false,
            generation: 0,
            work: None,
            resident: None,
        }
    }
}

impl Channel {
    pub(crate) fn retire(&mut self) {
        self.work = None;
        self.registers.values.recovery.fault_status = FaultStatusValues::new(0);
    }
    pub(crate) const fn suspend(&mut self) {
        self.registers.values.recovery.fault_status = FaultStatusValues::new(1);
    }

    pub(crate) fn latch(&mut self, fatal: ChannelFatal) {
        if self.registers.values.recovery.fatal.code() == Ok(ChannelFatalCode::None) {
            self.registers.values.recovery.fatal =
                self.registers.values.recovery.fatal.with_code(fatal.code());
            self.irq |= self.registers.values.irq.enable.raw() != 0;
            self.update_irq();
        }
    }

    pub(crate) fn update_irq(&mut self) {
        self.registers.values.irq.cause = ChannelIrqCauseValues::new(
            u32::from(!self.event.pointers().is_empty())
                | (self.registers.values.irq.cause.raw() & 6)
                | (u32::from(
                    self.registers.values.recovery.fatal.code() != Ok(ChannelFatalCode::None),
                ) << 3),
        );
    }

    pub(crate) fn report<M: hardware_interface_io::memory::MemoryAccess>(
        &mut self,
        work: crate::Work,
        event: &pdma_model_contract::dma::Event,
        memory: &M,
    ) -> crate::Error {
        if self.generation != work.generation || self.resetting() {
            return crate::Error::Reset;
        }
        if self.resident != Some(work) || self.work.is_none() {
            return crate::Error::Routing;
        }
        if self.halted() {
            return crate::Error::Halted;
        }
        self.stop();
        match self.event.push(memory, event) {
            Ok(()) => {
                self.irq |= self.registers.values.irq.enable.raw() != 0;
                self.update_irq();
                crate::Error::Halted
            }
            Err(fatal) => {
                self.latch(fatal);
                crate::Error::ChannelFatal(fatal)
            }
        }
    }

    pub(crate) const fn stop(&mut self) {
        self.registers.values.irq.cause =
            ChannelIrqCauseValues::new(self.registers.values.irq.cause.raw() | 4);
    }

    pub(crate) fn trap(&mut self) {
        self.registers.values.irq.cause =
            ChannelIrqCauseValues::new(self.registers.values.irq.cause.raw() | 2);
        self.irq |=
            self.registers.values.irq.enable.raw() != 0 && !self.registers.values.irq.mask.trap();
    }

    pub(crate) fn request_reset(&mut self) {
        if !self.resetting() {
            self.generation = self.generation.wrapping_add(1);
            self.admin.values.reset = ResetValues::new(1);
            if self.resident.is_none() {
                self.finish_reset();
            }
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
        match self.registers.values.recovery.fatal.code() {
            Ok(ChannelFatalCode::None) => self.registers.values.irq.cause.error(),
            Ok(
                ChannelFatalCode::CommandRead
                | ChannelFatalCode::EventWrite
                | ChannelFatalCode::EventFull
                | ChannelFatalCode::EventConfiguration,
            )
            | Err(_) => true,
        }
    }
    pub(crate) const fn enabled(&self) -> bool {
        self.registers.values.config.enable.raw() != 0
    }
}

#[derive(Debug)]
pub(crate) struct Accepted {
    pub(crate) command: Command,
    pub(crate) unread: Option<crate::Chunk>,
    pub(crate) payload: Option<Box<crate::Payload>>,
    pub(crate) progress: Option<CopyState>,
    pub(crate) fill_completed: u64,
    pub(crate) timestamp: Option<u64>,
}

/// Retained traversal of one command's two byte streams.
#[derive(Debug)]
pub(crate) struct CopyState {
    pub(crate) command: pdma_model_contract::dma::CopyCommand,
    source: Cursor,
    destination: Cursor,
    pub(crate) remaining: u64,
    bytes_completed: u64,
    pub(crate) status: Result<(), EventCode>,
}

#[derive(Debug)]
enum Side {
    Source,
    Destination,
}

/// Incremental validation of selected ranges, without payload access.
#[derive(Debug)]
pub struct RangeCheck {
    pub(crate) command: pdma_model_contract::dma::CopyCommand,
    cursor: Cursor,
    side: Side,
    remaining: u64,
}
