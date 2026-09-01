use super::{AsyncDeviceHandle, publish};
use crate::abi::ring::{PdmaBatchError, QueueBootstrapBatch, QueueResponseBytes, SubmissionBatch};

impl AsyncDeviceHandle {
    /// Applies one closed bootstrap batch under one lock after complete projection.
    pub async fn bootstrap_queue_batch(
        &self,
        request: QueueBootstrapBatch,
    ) -> Result<QueueResponseBytes, PdmaBatchError> {
        let committed = {
            let mut state = self.shared.state.lock().await;
            crate::abi::ring::batch::commit_bootstrap(&mut state, &request)?
        };
        let (response, generation) = committed.emit()?;
        publish(&self.shared, generation);
        Ok(response)
    }

    /// Applies one closed submission batch under one lock after complete projection.
    pub async fn submit_queue_batch(
        &self,
        request: SubmissionBatch,
    ) -> Result<QueueResponseBytes, PdmaBatchError> {
        let committed = {
            let mut state = self.shared.state.lock().await;
            crate::abi::ring::batch::commit_submission(&mut state, request.request())?
        };
        let (response, generation) = committed.emit()?;
        publish(&self.shared, generation);
        Ok(response)
    }
}
