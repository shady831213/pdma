use super::{PendingRetirement, RetirementStatus, RetirementUnit};
use crate::ModelError;
use crate::dma::PdmaMemoryMappings;
use crate::irq::PdmaIrqController;
use crate::queue::QueueBlock;
use crate::queue::io::NormalizedFault;
use crate::scheduler::Scheduler;
use crate::trace::TraceSink;
use pdma_model_contract::semantic::{
    Command, CommandId, CommandOperation, DeviceEpoch, IrqLine, IrqVector, QueueId, ResourceConfig,
    desc::CopyRequest,
    queue::{EventRouting, QueueConfig, SoftwareOwnerId},
};

#[test]
fn frontend_fault_cache_consumes_command_and_holds_source_for_retry() {
    // Given: one semantic queue containing a normalized command.
    let mut queues = QueueBlock::default();
    let queue = queues
        .create_semantic(
            DeviceEpoch::initial(),
            QueueConfig::new(2, 2, EventRouting::QueueLocal, SoftwareOwnerId::new(0)),
        )
        .expect("queue");
    queues
        .submit_semantic(
            queue.queue_id(),
            Command::new(
                CommandId::new(0),
                CommandOperation::Copy(CopyRequest::new(
                    pdma_model_contract::semantic::BufferId::new(0),
                    pdma_model_contract::semantic::BufferId::new(1),
                    1,
                )),
            ),
        )
        .expect("submission");
    let dma = PdmaMemoryMappings::new(std::sync::Arc::new(
        hardware_interface_runtime::SharedMemory::new(),
    ));
    let mut scheduler = Scheduler::default();
    let mut irq = PdmaIrqController::new(DeviceEpoch::initial(), IrqLine::new(IrqVector::new(0)));
    let mut trace = TraceSink::default();
    let mut unit = RetirementUnit::default();
    let source = ModelError::DmaSubmissionState;

    // When: a frontend fault is cached for retirement.
    let status = {
        let mut ports = super::RetirementPorts::new(
            &mut queues,
            &dma,
            &mut scheduler,
            &mut irq,
            &mut trace,
            ResourceConfig::default(),
        );
        unit.cache_frontend_fault(
            &mut ports,
            NormalizedFault {
                queue: queue.queue_id(),
                source,
            },
        )
    };

    // Then: the command is consumed and its typed source remains pending.
    assert_eq!(status, Ok(RetirementStatus::FaultCached));
    assert_eq!(queues.has_pending(queue.queue_id()), Ok(false));
    assert!(matches!(
        unit.pending(),
        Some(PendingRetirement::Fault {
            queue: pending_queue,
            source: Some(ModelError::DmaSubmissionState),
        }) if pending_queue == queue.queue_id()
    ));
}

#[test]
fn reset_clears_cached_retirement_state() {
    // Given: one cached fault retirement.
    let mut unit = RetirementUnit::default();
    unit.hold(PendingRetirement::Fault {
        queue: QueueId::new(2),
        source: Some(ModelError::DmaSubmissionState),
    });

    // When: the retirement hardware object resets.
    unit.reset();

    // Then: no cached record can publish after reset.
    assert!(!unit.is_pending());
    assert!(unit.pending().is_none());
}
