use super::exact::ExactQueueIo;
use super::{PdmaQueue, QueueIo};
use crate::ModelError;
use pdma_model_contract::semantic::{
    Command, CommandId, CommandOperation, CompletionRingId, QueueDirectoryEntry, QueueId,
    desc::CopyRequest,
    queue::{EventRouting, QueueConfig, SoftwareOwnerId},
};

#[test]
fn exact_queue_rejects_semantic_submission_without_consuming_state() {
    // Given: an exact queue whose ring state has not been activated.
    let queue_id = QueueId::new(3);
    let entry = QueueDirectoryEntry::new(
        queue_id,
        CompletionRingId::new(3),
        QueueConfig::new(2, 2, EventRouting::QueueLocal, SoftwareOwnerId::new(0)),
    );
    let mut queue = PdmaQueue::exact(entry, ExactQueueIo::inactive_for_test());
    let command = Command::new(
        CommandId::new(0),
        CommandOperation::Copy(CopyRequest::new(
            pdma_model_contract::semantic::BufferId::new(0),
            pdma_model_contract::semantic::BufferId::new(1),
            1,
        )),
    );

    // When: the semantic frontend attempts to submit into the exact queue object.
    let result = queue.io.submit_semantic(queue_id, command);

    // Then: the closed I/O mode rejects the command and keeps exact state empty.
    assert_eq!(result, Err(ModelError::UnknownQueue(queue_id)));
    assert!(matches!(queue.io, QueueIo::Exact(_)));
    assert!(!queue.io.has_pending());
}
