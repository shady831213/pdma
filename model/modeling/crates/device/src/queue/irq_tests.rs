use super::QueueBlock;
use super::io::PublicationOutcome;
use crate::test_support::{TestQueue, configure, irq_write, poll_completion};
use crate::{DeviceCore, PdmaMemoryProvisioning};
use pdma_model_contract::abi::AbiCompletion;
use pdma_model_contract::abi::registers::{IrqSourceRegisters, IrqStateBit, QueueRegisters};
use pdma_model_contract::semantic::{CompletionRingId, IrqSource, QueueId, ResourceConfig};

fn fixture(enabled: bool) -> (DeviceCore, TestQueue, TestQueue) {
    let mut provisioning = PdmaMemoryProvisioning::shared(std::sync::Arc::new(
        hardware_interface_runtime::SharedMemory::new(),
    ));
    let mut core = DeviceCore::new(provisioning.memory_access(), ResourceConfig::default());
    let queue = configure(&mut core, &mut provisioning, 0, [2, 1, 1]);
    let second = configure(&mut core, &mut provisioning, 1, [2, 1, 1]);
    irq_write(
        &mut core,
        queue,
        0,
        IrqSourceRegisters::ENABLED.offset(),
        u64::from(enabled),
    );
    (core, queue, second)
}

fn source_status(core: &DeviceCore, queue: TestQueue, source: u32) -> u64 {
    core.queues
        .read_irq(queue.index, source, IrqSourceRegisters::STATUS.offset())
        .expect("materialized source status")
}

#[test]
fn irq_sources_belong_to_materialized_queues_in_notification_order() {
    // Given: two hardware queues materialized by register configuration.
    let mut queues = QueueBlock::default();
    for index in 0..2 {
        queues
            .write_configuration(index, QueueRegisters::SQ_BASE.offset(), 0)
            .expect("queue configuration");
    }

    // When: the controller enumerates queue-owned IRQ sources.
    let sources: Vec<_> = queues.irq_sources().map(|(source, _)| source).collect();

    // Then: completion sources precede event sources, each in ascending queue order.
    assert_eq!(
        sources,
        [
            IrqSource::Completion(CompletionRingId::new(0)),
            IrqSource::Completion(CompletionRingId::new(1)),
            IrqSource::Event(QueueId::new(0)),
            IrqSource::Event(QueueId::new(1)),
        ]
    );
}

#[test]
fn completion_publication_latches_its_queue_source_without_retirement() {
    // Given: an enabled source and empty CQ configured through hardware registers.
    let (mut core, queue, _) = fixture(true);
    let mut trace = crate::trace::TraceSink::default();
    trace.enable();

    // When: the queue publishes a completion directly, without a retirement adapter.
    let result = core.queues.publish_completion(
        &core.memory,
        queue.queue_id(),
        AbiCompletion::success(0, 7),
        &mut trace,
    );

    // Then: the queue owns the record and latched cause, with exactly one latch trace.
    assert_eq!(result, Ok(PublicationOutcome::Published));
    assert_ne!(
        source_status(&core, queue, 0) & u64::from(IrqStateBit::CausePending.mask()),
        0
    );
    assert_ne!(
        source_status(&core, queue, 0) & u64::from(IrqStateBit::RecordsPending.mask()),
        0
    );
    assert_eq!(
        source_status(&core, queue, 1) & u64::from(IrqStateBit::CausePending.mask()),
        0
    );
    assert_eq!(
        trace
            .events()
            .iter()
            .filter(|event| event.kind()
                == pdma_model_contract::semantic::trace::TraceEventKind::IrqSourceLatched)
            .count(),
        1
    );
    assert_eq!(
        poll_completion(&mut core, queue),
        Some(AbiCompletion::success(0, 7))
    );
}

#[test]
fn disabled_publication_does_not_create_an_irq_cause() {
    // Given: a queue whose source retains its disabled reset value.
    let (mut core, queue, _) = fixture(false);
    let mut trace = crate::trace::TraceSink::default();

    // When: a completion becomes DMA-visible.
    core.queues
        .publish_completion(
            &core.memory,
            queue.queue_id(),
            AbiCompletion::success(0, 8),
            &mut trace,
        )
        .expect("completion publication");

    // Then: records are pending without synthesizing a source cause.
    let status = source_status(&core, queue, 0);
    assert_ne!(status & u64::from(IrqStateBit::RecordsPending.mask()), 0);
    assert_eq!(status & u64::from(IrqStateBit::CausePending.mask()), 0);
}

#[test]
fn event_publication_latches_only_its_queue_event_source() {
    // Given: a pending exact fault and an enabled event source.
    let (mut core, queue, _) = fixture(true);
    let mut trace = crate::trace::TraceSink::default();
    irq_write(&mut core, queue, 1, IrqSourceRegisters::ENABLED.offset(), 1);
    let fault = pdma_model_contract::abi::AbiFault::invalid_version(
        queue.layouts.submission().region().base(),
        0,
    );
    let event = pdma_model_contract::abi::AbiEvent::new(0, 17, fault);
    core.queues
        .stage_execution_fault(queue.queue_id(), event)
        .expect("pending event");

    // When: the queue publishes the event without a retirement adapter.
    let result = core
        .queues
        .publish_fault(&core.memory, queue.queue_id(), &mut trace);

    // Then: only the event source latches, and its exact event bytes are visible.
    assert!(matches!(result, Ok(super::io::FaultPublication::Exact(_))));
    assert_ne!(
        source_status(&core, queue, 1) & u64::from(IrqStateBit::CausePending.mask()),
        0
    );
    assert_eq!(
        source_status(&core, queue, 0) & u64::from(IrqStateBit::CausePending.mask()),
        0
    );
    assert_eq!(
        crate::test_support::poll_event(&mut core, queue),
        Some(event)
    );
}

#[test]
fn full_completion_ring_does_not_latch_a_second_message() {
    // Given: one message in a full one-entry CQ.
    let (mut core, queue, _) = fixture(false);
    let mut trace = crate::trace::TraceSink::default();
    irq_write(
        &mut core,
        queue,
        0,
        IrqSourceRegisters::ROUTE_KIND.offset(),
        1,
    );
    irq_write(&mut core, queue, 0, IrqSourceRegisters::ENABLED.offset(), 1);
    core.queues
        .publish_completion(
            &core.memory,
            queue.queue_id(),
            AbiCompletion::success(0, 0),
            &mut trace,
        )
        .expect("first completion");

    // When: publication retries against the full ring.
    let result = core.queues.publish_completion(
        &core.memory,
        queue.queue_id(),
        AbiCompletion::success(0, 1),
        &mut trace,
    );

    // Then: only the successful publication contributes a pending message.
    assert_eq!(result, Ok(PublicationOutcome::Retry));
    assert_eq!(
        core.queues
            .read_irq(0, 0, IrqSourceRegisters::PENDING_MESSAGES.offset()),
        Some(1)
    );
}

#[test]
fn line_ack_consults_only_its_own_queue_records() {
    // Given: two latched line sources, with only the second queue still holding a record.
    let (mut core, first, second) = fixture(true);
    let mut trace = crate::trace::TraceSink::default();
    irq_write(
        &mut core,
        second,
        0,
        IrqSourceRegisters::ENABLED.offset(),
        1,
    );
    core.queues
        .publish_completion(
            &core.memory,
            first.queue_id(),
            AbiCompletion::success(0, 0),
            &mut trace,
        )
        .expect("completion");
    assert!(poll_completion(&mut core, first).is_some());
    core.queues
        .publish_completion(
            &core.memory,
            second.queue_id(),
            AbiCompletion::success(1, 1),
            &mut trace,
        )
        .expect("other completion");

    // When: software acknowledges the drained source.
    irq_write(&mut core, first, 0, IrqSourceRegisters::ACK.offset(), 1);

    // Then: its cause clears independently of other queue storage.
    assert_eq!(
        source_status(&core, first, 0) & u64::from(IrqStateBit::CausePending.mask()),
        0
    );
}
