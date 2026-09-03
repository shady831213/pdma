use super::{AsyncDeviceHandle, wake};
use crate::dma::PdmaMemoryStage;
use crate::queue::exact::{PdmaControlError, QueueStatus, SubmissionAcceptance};
use pdma_model_contract::semantic::{QueueActivation, QueueControl};

impl AsyncDeviceHandle {
    /// Atomically commits three staged ring mappings and one queue activation.
    pub async fn activate_queue(
        &self,
        proposal: PdmaMemoryStage,
        activation: QueueActivation,
    ) -> Result<QueueStatus, PdmaControlError> {
        let (result, generation) = self
            .shared
            .backend
            .interact(|state| {
                let result = state.core.activate_staged_exact_queue(proposal, activation);
                if result.is_ok() {
                    wake(state);
                }
                (result, state.generation)
            })
            .await;
        if result.is_ok() {
            self.shared.backend.publish(generation);
        }
        result
    }

    /// Activates one queue over three exact mappings already committed in DMA space.
    pub async fn activate_mapped_queue(
        &self,
        activation: QueueActivation,
    ) -> Result<QueueStatus, PdmaControlError> {
        let (result, generation) = self
            .shared
            .backend
            .interact(|state| {
                let result = state.core.activate_mapped_exact_queue(activation);
                if result.is_ok() {
                    wake(state);
                }
                (result, state.generation)
            })
            .await;
        if result.is_ok() {
            self.shared.backend.publish(generation);
        }
        result
    }

    /// Atomically commits optional indirect DMA, opaque writes, and producer advance.
    pub async fn accept_submission(
        &self,
        request: SubmissionAcceptance,
    ) -> Result<QueueStatus, PdmaControlError> {
        let (result, generation) = self
            .shared
            .backend
            .interact(|state| {
                let result = state.core.accept_exact_submission(request);
                if result.is_ok() {
                    wake(state);
                }
                (result, state.generation)
            })
            .await;
        if result.is_ok() {
            self.shared.backend.publish(generation);
        }
        result
    }

    /// Executes one typed queue status or cursor transaction.
    pub async fn control_queue(
        &self,
        request: QueueControl,
    ) -> Result<QueueStatus, PdmaControlError> {
        let (result, changed, generation) = self
            .shared
            .backend
            .interact(|state| {
                let (result, changed) = state.core.control_exact_queue(request);
                if changed {
                    wake(state);
                }
                (result, changed, state.generation)
            })
            .await;
        if changed {
            self.shared.backend.publish(generation);
        }
        result
    }
}
