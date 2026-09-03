use super::{AsyncDeviceHandle, wake};
use crate::interface::batch::{
    PdmaBatchError, QueueBootstrapBatch, QueueResponseBytes, SubmissionBatch, commit_bootstrap,
    commit_submission,
};

impl AsyncDeviceHandle {
    /// Applies one closed bootstrap batch under one lock after complete projection.
    pub async fn bootstrap_queue_batch(
        &self,
        request: QueueBootstrapBatch,
    ) -> Result<QueueResponseBytes, PdmaBatchError> {
        let (committed, generation) = self
            .shared
            .backend
            .interact(|state| -> Result<_, PdmaBatchError> {
                let committed =
                    commit_bootstrap(&mut state.core, &request, &self.shared.register_gate)?;
                wake(state);
                Ok((committed, state.generation))
            })
            .await?;
        let response = committed.emit()?;
        self.shared.backend.publish(generation);
        Ok(response)
    }

    /// Applies one closed submission batch under one lock after complete projection.
    pub async fn submit_queue_batch(
        &self,
        request: SubmissionBatch,
    ) -> Result<QueueResponseBytes, PdmaBatchError> {
        let (committed, generation) = self
            .shared
            .backend
            .interact(|state| -> Result<_, PdmaBatchError> {
                let committed = commit_submission(
                    &mut state.core,
                    request.request(),
                    &self.shared.register_gate,
                )?;
                wake(state);
                Ok((committed, state.generation))
            })
            .await?;
        let response = committed.emit()?;
        self.shared.backend.publish(generation);
        Ok(response)
    }
}
