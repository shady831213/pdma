use hardware_interface_runtime::SharedMemory;
use pdma_model_device::{
    MappedDmaRegion, PdmaMemoryError, PdmaMemoryMappings, PdmaMemoryProvisioning, PdmaMemoryStage,
};
use std::{ops::Deref, sync::Arc};

pub(super) struct ProvisionedMemory {
    mappings: PdmaMemoryMappings,
    provisioning: PdmaMemoryProvisioning,
}

impl ProvisionedMemory {
    pub(super) fn new(memory: Arc<SharedMemory>) -> Self {
        let provisioning = PdmaMemoryProvisioning::shared(memory);
        Self {
            mappings: PdmaMemoryMappings::new(provisioning.memory_access()),
            provisioning,
        }
    }

    pub(super) fn allocate(&mut self, length: u64) -> Result<MappedDmaRegion, PdmaMemoryError> {
        self.provisioning.allocate(&mut self.mappings, length)
    }

    pub(super) fn stage(&self, lengths: &[u64]) -> Result<PdmaMemoryStage, PdmaMemoryError> {
        self.provisioning.stage(&self.mappings, lengths)
    }

    pub(super) fn commit(
        &mut self,
        stage: PdmaMemoryStage,
    ) -> Result<Vec<MappedDmaRegion>, PdmaMemoryError> {
        self.provisioning.commit(&mut self.mappings, stage)
    }
}

impl Deref for ProvisionedMemory {
    type Target = PdmaMemoryMappings;
    fn deref(&self) -> &Self::Target {
        &self.mappings
    }
}
