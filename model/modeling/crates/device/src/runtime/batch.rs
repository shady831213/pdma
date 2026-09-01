use super::{AsyncDeviceHandle, publish, wake};
use crate::abi::ring::{PdmaBatchError, QueueBootstrapBatch, QueueResponseBytes, SubmissionBatch};

impl AsyncDeviceHandle {
    /// Applies one closed bootstrap batch under one lock after complete projection.
    pub async fn bootstrap_queue_batch(
        &self,
        request: QueueBootstrapBatch,
    ) -> Result<QueueResponseBytes, PdmaBatchError> {
        let (committed, generation) = {
            let mut state = self.shared.state.lock().await;
            let committed = crate::abi::ring::batch::commit_bootstrap(
                &mut state.core,
                &request,
                &self.shared.register_gate,
            )?;
            wake(&mut state);
            (committed, state.generation)
        };
        let response = committed.emit()?;
        publish(&self.shared, generation);
        Ok(response)
    }

    /// Applies one closed submission batch under one lock after complete projection.
    pub async fn submit_queue_batch(
        &self,
        request: SubmissionBatch,
    ) -> Result<QueueResponseBytes, PdmaBatchError> {
        let (committed, generation) = {
            let mut state = self.shared.state.lock().await;
            let committed = crate::abi::ring::batch::commit_submission(
                &mut state.core,
                request.request(),
                &self.shared.register_gate,
            )?;
            wake(&mut state);
            (committed, state.generation)
        };
        let response = committed.emit()?;
        publish(&self.shared, generation);
        Ok(response)
    }
}
