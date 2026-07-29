use crate::dma_space::DmaSpace;
use pdma_model_contract::desc::ExternalCommand;
use pdma_model_contract::error::{ModelError, ModelResult};

#[derive(Debug)]
pub(in crate::runner::native) struct DmaBusQueueIo {
    decoded: Option<ExternalCommand>,
}

impl DmaBusQueueIo {
    pub(super) const fn new() -> Self {
        Self { decoded: None }
    }

    pub(super) fn submit_decoded(&mut self, command: ExternalCommand) -> ModelResult<()> {
        if self.decoded.is_some() {
            return Err(ModelError::RingFull);
        }
        self.decoded = Some(command);
        Ok(())
    }

    pub(super) fn fetch(&self, _dma_space: &DmaSpace) -> Option<ExternalCommand> {
        self.decoded.clone()
    }

    pub(super) fn consume(&mut self, _dma_space: &DmaSpace) {
        self.decoded = None;
    }

    pub(super) const fn has_pending(&self, _dma_space: &DmaSpace) -> bool {
        self.decoded.is_some()
    }
}
