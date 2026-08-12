use super::super::ring::NativeRing;
use crate::ModelResult;
use crate::dma_space::DmaSpace;
use pdma_model_contract::desc::ExternalCommand;

#[derive(Debug)]
pub(in crate::runner::native) struct SemanticQueueIo {
    commands: NativeRing<ExternalCommand>,
}

impl SemanticQueueIo {
    pub(super) fn new(depth: usize) -> ModelResult<Self> {
        NativeRing::with_capacity(depth).map(|commands| Self { commands })
    }

    pub(in crate::runner::native) fn submit(
        &mut self,
        command: ExternalCommand,
    ) -> ModelResult<()> {
        self.commands.push(command)
    }

    pub(super) fn fetch(&self, _dma_space: &DmaSpace) -> Option<ExternalCommand> {
        self.commands.front().cloned()
    }

    pub(super) fn consume(&mut self, _dma_space: &DmaSpace) {
        let _consumed = self.commands.pop();
    }

    pub(super) const fn has_pending(&self, _dma_space: &DmaSpace) -> bool {
        !self.commands.is_empty()
    }
}
