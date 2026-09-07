use hardware_interface_pipeline::{Operation, StepOutcome};
use pdma_model_contract::abi::{AbiCommand, AbiCompletion, AbiEvent, RingLayout};
use pdma_model_contract::semantic::{
    BufferId, CommandId, CommandOperation, CompletionRingId, DeviceEpoch, EventErrorKind,
    EventRecord, EventRouting, QueueActivation, QueueConfig, QueueId, QueueRingObjects,
    SoftwareOwnerId, TraceEventKind, desc::CopyRequest,
};

use super::support::{Case, PipelineRunOutcome, drive_to_complete, prepare, read};
use crate::ModelError;
use crate::device::{ModelStepStatus, hardware_context::HardwareContext};
use crate::execution::CommandRoute;
use crate::queue::io::NormalizedFault;
use crate::queue::stage::{FetchedCommand, FetchedWork};
use crate::retirement::stage::RetirementInput;

mod hardware;

use hardware::retire;

#[test]
fn frontend_normalization_fault_flows_through_fetch_and_retirement() {
    // Given: an exact queue whose captured descriptor has invalid ABI metadata.
    let mut fixture =
        super::support::Fixture::new(pdma_model_contract::semantic::ResourceConfig::default());
    fixture.core.enable_tracing();
    let stage = fixture
        .core
        .dma
        .stage(&[
            RingLayout::<AbiCommand>::checked_extent(2).expect("command extent"),
            RingLayout::<AbiCompletion>::checked_extent(1).expect("completion extent"),
            RingLayout::<AbiEvent>::checked_extent(1).expect("event extent"),
        ])
        .expect("ring stage");
    let [submission, completion, event] = stage.regions() else {
        panic!("three exact queue rings");
    };
    let (submission, completion, event) = (*submission, *completion, *event);
    let activation = QueueActivation::new(
        DeviceEpoch::initial(),
        QueueId::new(0),
        QueueConfig::new(2, 1, EventRouting::QueueLocal, SoftwareOwnerId::new(0)),
        QueueRingObjects::new(submission.object(), completion.object(), event.object()),
    );
    fixture
        .core
        .activate_staged_exact_queue(stage, activation)
        .expect("queue activation");
    fixture
        .core
        .dma
        .write_memory(submission.base(), &[0; AbiCommand::RECORD_SIZE])
        .expect("malformed descriptor");
    let queue = activation.queue();
    let active = fixture.core.exact_ring_mut(queue).expect("exact queue");
    active
        .publish_submission_producer(pdma_model_contract::abi::RingCursor::new(1))
        .expect("publish producer");
    active
        .capture_submission_producer(pdma_model_contract::abi::RingCursor::new(1))
        .expect("capture producer");
    fixture.core.ring_dma_bus_doorbell(queue).expect("doorbell");

    // When: fetch normalizes the descriptor violation and retirement publishes it.
    assert_eq!(
        drive_to_complete(&mut fixture),
        Ok(ModelStepStatus::FaultCached)
    );
    fixture.restart();
    assert_eq!(
        drive_to_complete(&mut fixture),
        Err(ModelError::DmaSubmissionState)
    );

    // Then: one SQ slot and one event slot advance with no success publication.
    let status = fixture
        .core
        .exact_ring(queue)
        .expect("exact queue")
        .status()
        .expect("queue status");
    assert_eq!(
        (
            status.submission().producer().raw(),
            status.submission().consumer().raw(),
            status.captured_submission_producer().raw(),
            status.completion().producer().raw(),
            status.event().producer().raw(),
        ),
        (1, 1, 1, 0, 1)
    );
    let published = fixture
        .core
        .dma
        .read_memory(event.base(), AbiEvent::RECORD_SIZE as u64)
        .expect("event bytes");
    assert_eq!(
        AbiEvent::decode(&published)
            .expect("canonical event")
            .fault()
            .kind(),
        pdma_model_contract::abi::AbiEventKind::InvalidVersion
    );
    assert_eq!(fixture.core.irq_snapshot(), None);
    assert_eq!(
        super::support::trace_kinds(&fixture.core.trace_events()),
        [TraceEventKind::EventPublished]
    );
    assert_eq!(fixture.step(), PipelineRunOutcome::Exhausted);
}

#[test]
fn frontend_fault_retirement_preserves_normalized_source_and_cursor() {
    // Given: normalized frontend failure state already staged by the queue frontend.
    let mut prepared = prepare(Case::Copy);
    let queue = prepared.queue.queue_id();
    let source = ModelError::UnknownBuffer(BufferId::new(88));
    let event = EventRecord::new(
        queue,
        CommandId::new(7),
        EventErrorKind::UnknownBuffer,
        BufferId::new(88),
    );
    assert_eq!(
        prepared
            .fixture
            .core
            .queues
            .stage_execution_fault(queue, event),
        Ok(source)
    );
    // When: the retirement owner caches the frontend failure, then retries it.
    assert_eq!(
        retire(
            &mut prepared.fixture.core,
            RetirementInput::FrontendFault(NormalizedFault { queue, source }),
        ),
        Ok(ModelStepStatus::FaultCached)
    );
    assert_eq!(
        prepared.fixture.core.queues.semantic_cursors(queue),
        Ok(((1, 1), (0, 0)))
    );
    let retired = retire(&mut prepared.fixture.core, RetirementInput::Retry);

    // Then: the exact source, one event, and one consumer advance are preserved.
    assert_eq!(retired, Err(source));
    assert_eq!(
        prepared.fixture.core.poll_queue_event(prepared.queue),
        Ok(Some(event))
    );
    assert_eq!(
        prepared.fixture.core.poll_queue_event(prepared.queue),
        Ok(None)
    );
    assert_eq!(
        prepared.fixture.core.queues.semantic_cursors(queue),
        Ok(((1, 1), (0, 0)))
    );
}

#[test]
fn invalid_completion_retires_without_dma_or_success_publication() {
    // Given: the execution hardware receives a route naming an absent completion ring.
    let mut prepared = prepare(Case::Copy);
    let source = prepared.fixture.core.dma.allocate(8).expect("source");
    prepared
        .fixture
        .core
        .dma
        .write_object(source.object(), 0, &[9; 8])
        .expect("source bytes");
    let route = CommandRoute::new(
        prepared.queue.queue_id(),
        CommandId::new(55),
        CompletionRingId::new(99),
        false,
    );
    let mut operation = Operation::new(FetchedWork::Command(FetchedCommand {
        route,
        operation: CommandOperation::Copy(CopyRequest::new(
            source.object(),
            prepared.destinations[0].object(),
            4,
        )),
    }));

    // When: execution validates the route and enters invalid-completion retirement.
    let core = &mut prepared.fixture.core;
    let input = {
        let mut context = HardwareContext {
            dma: &core.dma,
            trace: &mut core.trace,
            fetches: &mut core.native_fetches,
        };
        let input = loop {
            match operation.step(&mut core.execution, &mut context) {
                StepOutcome::Progressed => {}
                StepOutcome::Complete(Ok(input)) => break input,
                other => panic!("unexpected execution outcome: {other:?}"),
            }
        };
        assert!(matches!(
            operation.step(&mut core.execution, &mut context),
            StepOutcome::Exhausted
        ));
        input
    };
    let result = retire(core, input);

    // Then: the command consumes once without DMA, completion, event, or IRQ success.
    assert_eq!(
        result,
        Err(ModelError::UnknownCompletionRing(CompletionRingId::new(99)))
    );
    assert_eq!(
        read(&prepared.fixture.core, prepared.destinations[0]),
        Ok(vec![0; 8])
    );
    assert_eq!(
        prepared.fixture.core.poll_completion(prepared.queue),
        Ok(None)
    );
    assert_eq!(
        prepared.fixture.core.poll_queue_event(prepared.queue),
        Ok(None)
    );
    assert_eq!(prepared.fixture.core.irq_snapshot(), None);
    assert_eq!(
        prepared
            .fixture
            .core
            .queues
            .semantic_cursors(prepared.queue.queue_id()),
        Ok(((1, 1), (0, 0)))
    );
}
