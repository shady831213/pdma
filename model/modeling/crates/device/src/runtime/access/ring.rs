use crate::memory::PdmaMemoryStage;
use crate::queue::exact::{PdmaControlError, QueueStatus, SubmissionAcceptance};
use crate::runtime::AsyncDeviceHandle;
use pdma_model_contract::semantic::{QueueActivation, QueueControl};

impl AsyncDeviceHandle {
    /// Atomically commits three staged ring mappings and one queue activation.
    pub async fn activate_queue(
        &self,
        proposal: PdmaMemoryStage,
        activation: QueueActivation,
    ) -> Result<QueueStatus, PdmaControlError> {
        self.shared
            .backend
            .update(|state| {
                let result = state.core.activate_staged_exact_queue(proposal, activation);
                let changed = result.is_ok();
                (result, changed)
            })
            .await
    }

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

    /// Atomically commits optional indirect DMA, opaque writes, and producer advance.
    pub async fn accept_submission(
        &self,
        request: SubmissionAcceptance,
    ) -> Result<QueueStatus, PdmaControlError> {
        self.shared
            .backend
            .update(|state| {
                let result = state.core.accept_exact_submission(request);
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
