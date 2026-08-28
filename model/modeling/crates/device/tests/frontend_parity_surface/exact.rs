use crate::common::{TestResult, allocate, encode, fixture, run_descriptor};
use crate::operations::{OperationCase, command};
use pdma_model_contract::semantic::{ResourceConfig, TraceEventKind, TraceOperation};

pub(super) struct ExactObservation {
    pub(super) destinations: Vec<Vec<u8>>,
    pub(super) traces: Vec<(TraceEventKind, Option<TraceOperation>)>,
    pub(super) executions: usize,
    pub(super) retirements: usize,
}

pub(super) async fn observe(case: OperationCase) -> TestResult<ExactObservation> {
    let fixture = fixture(ResourceConfig::default()).await?;
    let source_a = allocate(&fixture.handle, 8).await?;
    let source_b = allocate(&fixture.handle, 8).await?;
    let destination_a = allocate(&fixture.handle, 8).await?;
    let destination_b = allocate(&fixture.handle, 8).await?;
    fixture
        .handle
        .write_memory(source_a.base(), &[1, 2, 3, 4, 5, 6, 7, 8])
        .await?;
    fixture
        .handle
        .write_memory(source_b.base(), &[8, 7, 6, 5, 4, 3, 2, 1])
        .await?;
    let (command, expected_a, expected_b) = command(
        case,
        &fixture.handle,
        source_a,
        source_b,
        destination_a,
        destination_b,
    )
    .await?;
    let result = run_descriptor(fixture, encode(command)?, &[destination_a, destination_b]).await?;
    assert_eq!(
        result.consumer,
        pdma_model_contract::semantic::RingCursor::new(1)
    );
    let retirements = usize::from(matches!(
        result.pending,
        Some(pdma_model_device::PendingDmaResult::Completion(_))
    ));
    assert_eq!(retirements, 1);
    assert_eq!(result.steps, 0);
    assert!(!result.irq_pending);
    assert_eq!(result.destinations, [expected_a, expected_b]);
    let _fault = crate::common::pending_event(&result);
    let executions = result.native_steps;
    let traces = result
        .traces
        .into_iter()
        .filter(|event| {
            matches!(
                event.kind(),
                TraceEventKind::Scheduled | TraceEventKind::ExecutionSucceeded
            )
        })
        .map(|event| (event.kind(), event.operation()))
        .collect();
    Ok(ExactObservation {
        destinations: result.destinations,
        traces,
        executions,
        retirements,
    })
}
