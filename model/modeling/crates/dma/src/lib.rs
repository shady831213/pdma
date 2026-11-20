//! Instance-configured channels sharing one DMA execution path.

mod access;
mod arbitration;
mod build;
/// Channel-owned hardware state, registers and rings.
pub mod channel;
mod error;
/// Borrowed resources for DMA execution stages.
pub mod execution_context;
mod flow;
mod maintenance;
mod memory;
mod reset;
mod stages;
#[cfg(test)]
mod tests;
mod work;

pub use arbitration::RoundRobin;
pub use build::{BuildError, DmaConfig, Timer};
pub use channel::{COPY_CHUNK_BYTES, Channel, CommandRing, EventRing};
pub use error::{ChannelFatal, Error};
pub use execution_context::ExecutionContext;
pub(crate) use execution_context::SelectedResources;
pub use flow::{
    Chunk, ChunkInput, CommandAdmission, CommandSnapshot, Completion, CopyProgress, FetchedCommand,
    Flow, Payload,
};
use hw_model_schema::RegisterWindow;
pub use maintenance::MaintenanceError;
pub use memory::{ChannelContext, MemoryFailure};
use pdma_model_contract::dma::{
    ChannelId, Command, ContextId, CopyCommandDestinationValues as Destination,
    CopyCommandSourceValues as Source, DmaAccess, DmaControl, DmaDiscovery, DmaFatalCode,
    DmaRegistersLayout,
};
pub use stages::{
    Candidate, CheckRanges, CommandExecution, CommandRequest, CopyChunk, Execution, Fetch,
    Hardware, Pipeline, PrepareChunk, ReadCopy, RetainedTask, Retire, WriteCopy,
};
pub use work::Work;

#[derive(Debug)]
struct Frontier {
    channel: ChannelId,
    generation: u64,
    consumer: u32,
    pending: u32,
    passed: bool,
    failed: bool,
    referenced: bool,
}

#[derive(Debug)]
struct Maintenance {
    context: ContextId,
    abort: bool,
    frontiers: Vec<Frontier>,
    result: Result<bool, MaintenanceError>,
}

const fn references(command: &Command) -> [Option<ContextId>; 3] {
    let copy = match command {
        Command::Copy(copy) => copy,
        Command::Write(command) => {
            return [
                Some(ContextId::new(command.word.target.context)),
                None,
                None,
            ];
        }
        Command::Fence(command) => {
            return [
                Some(ContextId::new(command.word.target.context)),
                None,
                None,
            ];
        }
        Command::Wait(wait) => {
            return [Some(ContextId::new(wait.target.context)), None, None];
        }
        Command::Atomic(atomic) => {
            return [Some(ContextId::new(atomic.target.context)), None, None];
        }
        Command::Semaphore(semaphore) => {
            return [Some(ContextId::new(semaphore.target.context)), None, None];
        }
        Command::Fill(fill) => {
            return [Some(ContextId::new(fill.context)), None, None];
        }
        Command::Timestamp(timestamp) => {
            return [Some(ContextId::new(timestamp.context)), None, None];
        }
        Command::Trap(_) => return [None, None, None],
    };
    let sg = match (copy.source, copy.destination) {
        (Source::Linear(_), Destination::Linear(_)) => None,
        (Source::ScatterGather(_), _) | (_, Destination::ScatterGather(_)) => {
            Some(ContextId::new(copy.sg_context))
        }
    };
    [
        Some(ContextId::new(copy.source_context)),
        Some(ContextId::new(copy.destination_context)),
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
    active: Option<Work>,
    maintenance: Option<Maintenance>,
    timer: Option<std::sync::Arc<dyn Timer>>,
}

impl<M, S> Dma<M, S> {
    /// Active-high IRQ levels in physical channel order.
    pub fn channel_irqs(&self) -> impl ExactSizeIterator<Item = bool> + '_ {
        self.channels.iter().map(Channel::irq)
    }

    /// Level of the selected channel's IRQ output.
    pub fn channel_irq(&self, id: ChannelId) -> bool {
        self.channels.get(id.index()).is_some_and(Channel::irq)
    }

    /// Level of the independent device-fatal IRQ output.
    pub fn device_irq(&self) -> bool {
        self.control.values.irq_enable.raw() != 0
            && self.control.values.fatal.code() != Ok(DmaFatalCode::None)
    }

    /// Whether any DMA interrupt output is asserted.
    pub fn irq(&self) -> bool {
        self.device_irq() || self.channel_irqs().any(|level| level)
    }

    /// Signals a device-wide internal hardware fault, never a software bus command.
    pub fn signal_fatal(&mut self) {
        if self.control.values.fatal.code() == Ok(DmaFatalCode::None) {
            self.control.values.fatal = self.control.values.fatal.with_code(DmaFatalCode::Internal);
            self.refresh_maintenance();
            for channel in &mut self.channels {
                if channel.resident.is_none() {
                    channel.retire();
                }
            }
            self.release_invalid_grant();
        }
    }

    pub(crate) fn halted(&self) -> bool {
        self.control.values.fatal.code() != Ok(DmaFatalCode::None)
    }

    pub(crate) fn resetting(&self) -> bool {
        self.control.values.reset.request()
    }
}
