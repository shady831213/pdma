use super::{AsyncDeviceHandle, publish, wake};
use crate::abi::ring::{PdmaControlError, QueueStatus, SubmissionAcceptance, control};
use crate::dma::PdmaMemoryStage;
use pdma_model_contract::semantic::{QueueActivation, QueueControl};

impl AsyncDeviceHandle {
    /// Atomically commits three staged ring mappings and one queue activation.
    pub async fn activate_queue(
        &self,
        proposal: PdmaMemoryStage,
        activation: QueueActivation,
    ) -> Result<QueueStatus, PdmaControlError> {
        let (result, generation) = {
            let mut state = self.shared.state.lock().await;
            let result = control::activate_staged(&mut state.core, proposal, activation);
            if result.is_ok() {
                wake(&mut state);
            }
            (result, state.generation)
        };
        if result.is_ok() {
            publish(&self.shared, generation);
        }
        result
    }

    /// Activates one queue over three exact mappings already committed in DMA space.
    pub async fn activate_mapped_queue(
        &self,
        activation: QueueActivation,
    ) -> Result<QueueStatus, PdmaControlError> {
        let (result, generation) = {
            let mut state = self.shared.state.lock().await;
            let result = control::activate_mapped(&mut state.core, activation);
            if result.is_ok() {
                wake(&mut state);
            }
            (result, state.generation)
        };
        if result.is_ok() {
            publish(&self.shared, generation);
        }
        result
    }

    /// Atomically commits optional indirect DMA, opaque writes, and producer advance.
    pub async fn accept_submission(
        &self,
        request: SubmissionAcceptance,
    ) -> Result<QueueStatus, PdmaControlError> {
        let (result, generation) = {
            let mut state = self.shared.state.lock().await;
            let result = control::transactions::accept_submission(&mut state.core, request);
            if result.is_ok() {
                wake(&mut state);
            }
            (result, state.generation)
        };
        if result.is_ok() {
            publish(&self.shared, generation);
        }
        result
    }

    /// Executes one typed queue status or cursor transaction.
    pub async fn control_queue(
        &self,
        request: QueueControl,
    ) -> Result<QueueStatus, PdmaControlError> {
        let (result, changed, generation) = {
            let mut state = self.shared.state.lock().await;
            let (result, changed) = control::transactions::control(&mut state.core, request);
            if changed {
                wake(&mut state);
            }
            (result, changed, state.generation)
        };
        if changed {
            publish(&self.shared, generation);
        }
        result
    }
}
