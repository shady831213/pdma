mod dma_bus;
mod memory;
mod semantic;

use crate::dma_space::DmaSpace;
use pdma_model_contract::desc::ExternalCommand;
use pdma_model_contract::error::{ModelError, ModelResult};
use pdma_model_contract::ids::{CommandId, QueueId};

pub(in crate::runner::native) use memory::QueueMemoryResolver;

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

    pub(super) fn submit_decoded_dma(
        &mut self,
        queue_id: QueueId,
        command: ExternalCommand,
    ) -> ModelResult<()> {
        match self {
            Self::Semantic(_) => Err(ModelError::UnknownQueue(queue_id)),
            Self::DmaBus(io) => io.submit_decoded(command),
        }
    }

    pub(super) fn fetch(&self, dma_space: &DmaSpace) -> Option<ExternalCommand> {
        match self {
            Self::Semantic(io) => io.fetch(dma_space),
            Self::DmaBus(io) => io.fetch(dma_space),
        }
    }

    pub(super) fn consume(&mut self, dma_space: &DmaSpace) {
        match self {
            Self::Semantic(io) => io.consume(dma_space),
            Self::DmaBus(io) => io.consume(dma_space),
        }
    }

    pub(super) const fn has_pending(&self, dma_space: &DmaSpace) -> bool {
        match self {
            Self::Semantic(io) => io.has_pending(dma_space),
            Self::DmaBus(io) => io.has_pending(dma_space),
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
