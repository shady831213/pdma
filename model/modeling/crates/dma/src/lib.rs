//! Instance-configured channels sharing one DMA execution path.

mod access;
mod arbitration;
mod build;
mod channel;
mod error;
mod execution;
mod fetch;
mod maintenance;
mod memory;
mod reset;
mod routing;
mod stages;
#[cfg(test)]
mod tests;

pub use arbitration::{Candidate, CommandArbiter, CommandArbiterState, Intake, RoundRobin};
pub use build::BuildError;
pub use channel::{Channel, CommandRing, ResponseRing};
pub use error::{ChannelFatal, Error};
pub use execution::{
    COPY_CHUNK_BYTES, CheckRanges, CopyState, NextChunk, RangeCheck, ReadCopy, Transfer, WriteCopy,
};
pub use fetch::Fetch;
use hardware_interface_schema::RegisterWindow;
pub use maintenance::MaintenanceError;
pub use memory::{ChannelContext, MemoryFailure};
use pdma_model_contract::dma::{
    ChannelId, Command, ContextId, DmaAccess, DmaControl, DmaDiscovery, DmaFatalCode,
    DmaRegistersLayout, EndpointMode, ResetValues,
};
pub use reset::{ChannelTask, ResetControl, Resettable};
pub use routing::{ResponsePort, Responses, RouteResponse};
pub use stages::{CommandLoop, CopyLoop, Execution, Pipeline, Publish, RunCommand, UpdateIrq};

#[derive(Debug)]
struct Frontier {
    channel: ChannelId,
    generation: u64,
    consumer: u32,
    pending: u32,
    passed: bool,
    failed: bool,
}

#[derive(Debug)]
struct Maintenance {
    context: ContextId,
    abort: bool,
    frontiers: Vec<Frontier>,
    result: Result<bool, MaintenanceError>,
}

const fn references(command: &Command) -> [Option<ContextId>; 3] {
    let sg = match (command.source.mode, command.destination.mode) {
        (EndpointMode::Linear, EndpointMode::Linear) => None,
        (EndpointMode::ScatterGather, _) | (_, EndpointMode::ScatterGather) => {
            Some(ContextId::new(command.sg_context))
        }
    };
    [
        Some(ContextId::new(command.source.context)),
        Some(ContextId::new(command.destination.context)),
        sg,
    ]
}

/// Hardware context; the assembler chooses and owns the memory backing.
#[derive(Debug)]
pub struct Dma<M, S = ()> {
    layout: DmaRegistersLayout,
    discovery: RegisterWindow<DmaDiscovery>,
    channels: Vec<Channel>,
    /// Injected memory access capability, never a device-selected backend.
    pub memory: M,
    sideband: fn(DmaAccess) -> S,
    control: RegisterWindow<DmaControl>,
    interrupt: bool,
    active: Option<ChannelTask<[Option<pdma_model_contract::dma::ContextId>; 3]>>,
    maintenance: Option<Maintenance>,
}

impl<M, S> Dma<M, S> {
    /// Whether hardware has emitted a notification not yet taken by the platform.
    pub fn irq(&self) -> bool {
        self.channels.iter().any(|channel| channel.irq) || self.interrupt
    }

    /// Transfers coalesced notification ownership to the platform interrupt controller.
    pub fn take_interrupt(&mut self) -> bool {
        let mut pending = std::mem::take(&mut self.interrupt);
        for channel in &mut self.channels {
            pending |= std::mem::take(&mut channel.irq);
        }
        pending
    }

    /// Takes only this channel's coalesced pulse for platform-specific routing.
    pub fn take_channel_interrupt(&mut self, id: ChannelId) -> bool {
        self.channels
            .get_mut(id.index())
            .is_some_and(|channel| std::mem::take(&mut channel.irq))
    }

    /// Takes the device-wide source independently of channel notifications.
    pub fn take_device_interrupt(&mut self) -> bool {
        std::mem::take(&mut self.interrupt)
    }

    /// Signals a device-wide internal hardware fault, never a software bus command.
    pub fn signal_fatal(&mut self) {
        if self.control.values.fatal.code() == Ok(DmaFatalCode::None) {
            self.control.values.fatal = self.control.values.fatal.with_code(DmaFatalCode::Internal);
            self.interrupt |= self.control.values.irq_enable.raw() != 0;
        }
    }

    pub(crate) fn halted(&self) -> bool {
        self.control.values.fatal.code() != Ok(DmaFatalCode::None)
    }

    pub(crate) fn resetting(&self) -> bool {
        self.control.values.reset.request()
    }

    pub(crate) fn request_reset(&mut self) {
        if !self.resetting() {
            self.control.values.reset = ResetValues::new(1);
            for channel in &mut self.channels {
                channel.request_reset();
            }
        }
    }
}
