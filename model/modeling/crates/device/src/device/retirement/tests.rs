use super::{PendingRetirement, RetirementStatus, RetirementUnit};
use crate::ModelError;
use crate::device::context::HardwareContext;
use crate::queue::io::NormalizedFault;
use crate::queue::runnable::RunnableQueues;
use crate::trace::TraceSink;
use pdma_model_contract::semantic::{QueueId, ResourceConfig};

#[test]
fn frontend_fault_cache_consumes_command_and_holds_source_for_retry() {
    // Given: one exact queue with a published and captured descriptor.
    let allocator = hardware_interface_runtime::SharedMemoryAllocator::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        hardware_interface_runtime::MemoryAddress::new(0x1_0000),
    );
    let mut core = crate::DeviceCore::new(allocator.memory_access(), ResourceConfig::default());
    let queue = crate::test_support::configure(&mut core, &allocator, 0, [2, 2, 2]);
    crate::test_support::submit(&mut core, queue, crate::test_support::copy(0, 0, 0, 1));
    crate::test_support::doorbell(&mut core, queue);
    let mut trace = TraceSink::default();
    let mut runnable = RunnableQueues::default();
    let mut fetches = 0;
    let mut unit = RetirementUnit::default();
    let source = ModelError::DmaSubmissionState;

    // When: a frontend fault is cached for retirement.
    let status = {
        let mut context = HardwareContext {
            queues: &mut core.queues,
            runnable: &mut runnable,
            memory: &core.memory,
            trace: &mut trace,
            fetches: &mut fetches,
            resources: ResourceConfig::default(),
        };
        let mut ports = super::RetirementPorts::new(&mut context);
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
    assert_eq!(core.queues.has_pending(queue.queue_id()), Ok(false));
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
