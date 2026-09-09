use hardware_interface_pipeline::{Operation, StepOutcome};
use pdma_model_contract::abi::{AbiCommand, AbiEventKind, RingCursor};
use pdma_model_contract::semantic::{CommandId, CompletionRingId, ResourceConfig, TraceEventKind};

use super::support::{Case, Fixture, PipelineRunOutcome, drive_to_complete, prepare, read};
use crate::ModelError;
use crate::device::flow::{CommandRoute, FetchedCommand, FetchedWork, RetirementWork};
use crate::device::retirement::stage::RetirementState;
use crate::device::{ModelStepStatus, context::HardwareContext};
use crate::operation::{CopyOperands, DecodedOperation};
use crate::queue::io::NormalizedFault;

mod hardware;

use hardware::retire;

#[test]
fn frontend_normalization_fault_flows_through_fetch_and_retirement() {
    // Given: an exact queue whose captured descriptor has invalid ABI metadata.
    let mut fixture = Fixture::new(ResourceConfig::default());
    fixture.core.enable_tracing();
    let queue = crate::test_support::configure(&mut fixture.core, &fixture.allocator, 0, [2, 1, 1]);
    let address = queue
        .layouts
        .submission()
        .entry_address(RingCursor::new(0))
        .expect("SQ slot");
    crate::memory::write(&fixture.core.memory, address, &[0; AbiCommand::RECORD_SIZE])
        .expect("invalid descriptor");
    crate::test_support::write_queue(
        &mut fixture.core,
        queue,
        pdma_model_contract::abi::registers::QueueRegisters::SQ_PRODUCER.offset(),
        1,
    );
    crate::test_support::doorbell(&mut fixture.core, queue);

    // When: fetch normalizes the descriptor violation and retirement publishes it.
    assert_eq!(
        drive_to_complete(&mut fixture),
        Ok(ModelStepStatus::FaultCached)
    );
    assert!(fixture.core.command_pipeline.is_empty());
    assert!(fixture.core.command_pipeline.retirement.is_pending());
    assert_eq!(fixture.fetch_count(), 1);
    assert_eq!(
        super::support::cursors(&fixture.core, queue),
        ((1, 1), (0, 0))
    );
    assert_eq!(
        crate::test_support::poll_event(&mut fixture.core, queue),
        None
    );
    let cached_trace = fixture.core.trace_events();
    assert_eq!(fixture.step(), PipelineRunOutcome::Exhausted);
    assert_eq!(fixture.core.trace_events(), cached_trace);
    fixture.restart();
    assert_eq!(
        drive_to_complete(&mut fixture),
        Err(ModelError::MemoryFault(
            pdma_model_contract::abi::AbiFault::invalid_version(address, 0)
        ))
    );
    assert_eq!(fixture.fetch_count(), 1);
    assert!(fixture.core.command_pipeline.is_empty());
    assert!(!fixture.core.command_pipeline.retirement.is_pending());

    // Then: one SQ slot and one event slot advance with no success publication.
    assert_eq!(
        super::support::cursors(&fixture.core, queue),
        ((1, 1), (0, 0))
    );
    let event = crate::test_support::poll_event(&mut fixture.core, queue).expect("fault event");
    assert_eq!(event.fault().kind(), AbiEventKind::InvalidVersion);
    assert_eq!(fixture.core.irq_snapshot(), None);
    assert_eq!(
        super::support::trace_kinds(&fixture.core.trace_events()),
        [
            TraceEventKind::DoorbellRunnable,
            TraceEventKind::EventPublished,
            TraceEventKind::EventPolled
        ]
    );
    assert_eq!(fixture.step(), PipelineRunOutcome::Exhausted);
}

#[test]
fn frontend_fault_retirement_preserves_normalized_source_and_cursor() {
    // Given: normalized frontend failure state already staged by the queue frontend.
    let mut prepared = prepare(Case::Copy);
    let queue = prepared.queue.queue_id();
    let address = prepared
        .queue
        .layouts
        .submission()
        .entry_address(RingCursor::new(0))
        .expect("descriptor");
    crate::memory::write(&prepared.fixture.core.memory, address, &0_u16.to_le_bytes())
        .expect("invalid version");
    let source = ModelError::DmaSubmissionState;
    let fetched = prepared
        .fixture
        .core
        .queues
        .fetch(&prepared.fixture.core.memory, queue)
        .expect("fetch");
    assert!(matches!(
        fetched,
        Some(crate::queue::io::NormalizedFetch::Fault(_))
    ));
    assert_eq!(
        retire(
            &mut prepared.fixture.core,
            RetirementState::Work(RetirementWork::FrontendFault(NormalizedFault {
                queue,
                source
            }))
        ),
        Ok(ModelStepStatus::FaultCached)
    );
    assert_eq!(
        super::support::cursors(&prepared.fixture.core, prepared.queue),
        ((1, 1), (0, 0))
    );
    let retired = retire(&mut prepared.fixture.core, RetirementState::Retry);
    assert_eq!(retired, Err(source));
    let event =
        crate::test_support::poll_event(&mut prepared.fixture.core, prepared.queue).expect("event");
    assert_eq!(event.command_id, 7);
    assert_eq!(event.fault().kind(), AbiEventKind::InvalidVersion);
    assert_eq!(event.fault_locator, address.raw());
    assert_eq!(
        crate::test_support::poll_event(&mut prepared.fixture.core, prepared.queue),
        None
    );
    assert_eq!(
        super::support::cursors(&prepared.fixture.core, prepared.queue),
        ((1, 1), (0, 0))
    );
}

#[test]
fn invalid_completion_retires_without_dma_or_success_publication() {
    // Given: the execution hardware receives a route naming an absent completion ring.
    let mut prepared = prepare(Case::Copy);
    let source = crate::test_support::allocate(&prepared.fixture.allocator, 8);
    crate::memory::write(&prepared.fixture.core.memory, source.base(), &[9; 8])
        .expect("source bytes");
    let route = CommandRoute::new(
        prepared.queue.queue_id(),
        CommandId::new(55),
        CompletionRingId::new(99),
        false,
    );
    let mut operation = Operation::new(FetchedWork::Command(FetchedCommand {
        route,
        operation: DecodedOperation::Copy(CopyOperands {
            source: source.base(),
            destination: prepared.destinations[0].base(),
            length: 4,
        }),
    }));

    // When: execution validates the route and enters invalid-completion retirement.
    let core = &mut prepared.fixture.core;
    let input = {
        let mut context = HardwareContext {
            queues: &mut core.queues,
            runnable: &mut core.runnable,
            memory: &core.memory,
            trace: &mut core.trace,
            fetches: &mut core.native_fetches,
            resources: core.command_pipeline.execution.resources(),
        };
        let input = loop {
            match operation.step(&mut core.command_pipeline.execution, &mut context) {
                StepOutcome::Progressed => {}
                StepOutcome::Complete(Ok(input)) => break input,
                other => panic!("unexpected execution outcome: {other:?}"),
            }
        };
        assert!(matches!(
            operation.step(&mut core.command_pipeline.execution, &mut context),
            StepOutcome::Exhausted
        ));
        input
    };
    let result = retire(core, RetirementState::Work(input));

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
        crate::test_support::poll_completion(&mut prepared.fixture.core, prepared.queue),
        None
    );
    assert_eq!(
        crate::test_support::poll_event(&mut prepared.fixture.core, prepared.queue),
        None
    );
    assert_eq!(prepared.fixture.core.irq_snapshot(), None);
    assert_eq!(
        crate::device::command_pipeline::tests::support::cursors(
            &prepared.fixture.core,
            prepared.queue
        ),
        ((1, 1), (0, 0))
    );
}
