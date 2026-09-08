use crate::queue::exact::{PdmaControlError, QueueStatus};
use crate::runtime::AsyncDeviceHandle;
use pdma_model_contract::semantic::{QueueActivation, QueueControl};

impl AsyncDeviceHandle {
    /// Activates one queue over three exact mappings already committed in DMA space.
    pub async fn activate_mapped_queue(
        &self,
        activation: QueueActivation,
    ) -> Result<QueueStatus, PdmaControlError> {
        self.shared
            .backend
            .update(|state| {
                let result = state.core.activate_mapped_exact_queue(activation);
                let changed = result.is_ok();
                (result, changed)
            })
            .await
    }

    /// Executes one typed queue status or cursor transaction.
    pub async fn control_queue(
        &self,
        request: QueueControl,
    ) -> Result<QueueStatus, PdmaControlError> {
        self.shared
            .backend
            .update(|state| state.core.control_exact_queue(request))
            .await
    }
}
