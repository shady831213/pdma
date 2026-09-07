use crate::interface::batch::{
    CommittedBatch, PdmaBatchError, QueueBootstrapBatch, QueueResponseBytes, SubmissionBatch,
    commit_bootstrap, commit_submission,
};
use crate::runtime::AsyncDeviceHandle;

impl AsyncDeviceHandle {
    /// Applies one closed bootstrap batch under one lock after complete projection.
    pub async fn bootstrap_queue_batch(
        &self,
        request: QueueBootstrapBatch,
    ) -> Result<QueueResponseBytes, PdmaBatchError> {
        self.shared
            .backend
            .commit(
                |state| -> Result<_, PdmaBatchError> {
                    let committed =
                        commit_bootstrap(&mut state.core, &request, &self.shared.register_gate)?;
                    Ok((committed, true))
                },
                CommittedBatch::emit,
            )
            .await
    }

    /// Applies one closed submission batch under one lock after complete projection.
    pub async fn submit_queue_batch(
        &self,
        request: SubmissionBatch,
    ) -> Result<QueueResponseBytes, PdmaBatchError> {
        self.shared
            .backend
            .commit(
                |state| -> Result<_, PdmaBatchError> {
                    let committed = commit_submission(
                        &mut state.core,
                        request.request(),
                        &self.shared.register_gate,
                    )?;
                    Ok((committed, true))
                },
                CommittedBatch::emit,
            )
            .await
    }
}
