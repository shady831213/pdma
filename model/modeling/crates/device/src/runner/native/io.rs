mod dma_bus;
mod memory;
mod semantic;

use crate::dma_space::DmaSpace;
use crate::runner::bus::control::BusControlState;
use pdma_model_contract::abi::AbiEvent;
use pdma_model_contract::desc::ExternalCommand;
use pdma_model_contract::error::ModelResult;
use pdma_model_contract::ids::{CommandId, QueueId};

pub(in crate::runner::native) use memory::QueueMemoryResolver;

#[derive(Debug, Clone)]
pub(super) enum QueueFetch {
    Command(ExternalCommand),
    Fault(AbiEvent),
}

#[derive(Debug)]
pub(super) enum QueueIo {
    Semantic(semantic::SemanticQueueIo),
    DmaBus(dma_bus::DmaBusQueueIo),
}

impl QueueIo {
    pub(super) fn semantic(depth: usize) -> ModelResult<Self> {
        semantic::SemanticQueueIo::new(depth).map(Self::Semantic)
    }

    pub(super) const fn dma_bus() -> Self {
        Self::DmaBus(dma_bus::DmaBusQueueIo::new())
    }

    pub(super) const fn semantic_mut(&mut self) -> Option<&mut semantic::SemanticQueueIo> {
        match self {
            Self::Semantic(io) => Some(io),
            Self::DmaBus(_) => None,
        }
    }

    pub(super) fn fetch(
        &self,
        dma_space: &DmaSpace,
        bus_control: &BusControlState,
        queue_id: QueueId,
        completion_ring: pdma_model_contract::CompletionRingId,
    ) -> ModelResult<Option<QueueFetch>> {
        match self {
            Self::Semantic(io) => Ok(io.fetch(dma_space).map(QueueFetch::Command)),
            Self::DmaBus(_) => {
                dma_bus::DmaBusQueueIo::fetch(dma_space, bus_control, queue_id, completion_ring)
            }
        }
    }

    pub(super) fn consume(
        &mut self,
        dma_space: &DmaSpace,
        bus_control: &mut BusControlState,
        queue_id: QueueId,
    ) -> ModelResult<()> {
        match self {
            Self::Semantic(io) => {
                io.consume(dma_space);
                Ok(())
            }
            Self::DmaBus(_) => dma_bus::DmaBusQueueIo::consume(bus_control, queue_id),
        }
    }

    pub(super) fn has_pending(
        &self,
        dma_space: &DmaSpace,
        bus_control: &BusControlState,
        queue_id: QueueId,
    ) -> bool {
        match self {
            Self::Semantic(io) => io.has_pending(dma_space),
            Self::DmaBus(_) => dma_bus::DmaBusQueueIo::has_pending(bus_control, queue_id),
        }
    }

    pub(super) const fn publication_requires_retry(&self) -> bool {
        match self {
            Self::Semantic(_) => false,
            Self::DmaBus(_) => true,
        }
    }

    pub(super) const fn memory_resolver(
        &self,
        queue_id: QueueId,
        command_id: CommandId,
    ) -> QueueMemoryResolver {
        match self {
            Self::Semantic(_) => QueueMemoryResolver::semantic(queue_id, command_id),
            Self::DmaBus(_) => QueueMemoryResolver::dma_bus(queue_id, command_id),
        }
    }
}
