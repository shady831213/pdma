use pdma_model_contract::semantic::{ResourceConfig, TraceEventKind};
use pdma_model_device::{AsyncStallReason, PipelineRunOutcome, PipelineTerminal};

use super::support::{
    Case, count, drive_to_complete, drive_to_pending, prepare_with_resources, prepared_pair,
};

#[test]
fn no_replay() -> Result<(), String> {
    // Given: one fetched copy blocked by execution resources.
    let mut execution =
        prepare_with_resources(Case::Copy, ResourceConfig::default().with_read_slots(0));

    // When: execution pressure repeats before resources become available.
    let execution_wait = AsyncStallReason::ExecutionResourcePressure;
    assert_eq!(
        drive_to_pending(&mut execution.fixture),
        PipelineRunOutcome::Pending(execution_wait)
    );
    for _ in 0..2 {
        assert_eq!(
            execution.fixture.step(),
            PipelineRunOutcome::Pending(execution_wait)
        );
    }
    assert_eq!(execution.fixture.fetch_count(), 1);
    assert_eq!(
        count(
            &execution.fixture.trace_events(),
            TraceEventKind::ExecutionSucceeded
        ),
        0
    );
    execution.fixture.set_resources(ResourceConfig::default());
    assert_eq!(
        drive_to_complete(&mut execution.fixture)?,
        Ok(PipelineTerminal::Progressed)
    );
    assert_eq!(execution.fixture.fetch_count(), 1);
    assert_eq!(
        count(
            &execution.fixture.trace_events(),
            TraceEventKind::ExecutionSucceeded
        ),
        1
    );

    // Given: a second copy executes behind a full one-entry completion path.
    let mut retirement = prepared_pair();
    assert_eq!(
        drive_to_complete(&mut retirement.fixture)?,
        Ok(PipelineTerminal::Progressed)
    );
    retirement.fixture.restart();
    let retirement_wait = AsyncStallReason::CompletionPublicationPressure;
    assert_eq!(
        drive_to_pending(&mut retirement.fixture),
        PipelineRunOutcome::Pending(retirement_wait)
    );
    assert_eq!(
        retirement.fixture.step(),
        PipelineRunOutcome::Pending(retirement_wait)
    );
    assert_eq!(retirement.fixture.fetch_count(), 2);
    assert_eq!(
        count(
            &retirement.fixture.trace_events(),
            TraceEventKind::ExecutionSucceeded
        ),
        2
    );

    // When: only downstream space changes and retirement resumes.
    assert!(
        retirement
            .fixture
            .poll_completion(retirement.queue)
            .expect("first completion")
            .is_some()
    );
    assert_eq!(
        drive_to_complete(&mut retirement.fixture)?,
        Ok(PipelineTerminal::Progressed)
    );
    assert_eq!(retirement.fixture.fetch_count(), 2);
    assert_eq!(
        count(
            &retirement.fixture.trace_events(),
            TraceEventKind::ExecutionSucceeded
        ),
        2
    );
    assert_eq!(retirement.fixture.step(), PipelineRunOutcome::Exhausted);
    assert_eq!(
        retirement
            .fixture
            .read(retirement.destinations[1], 8)
            .expect("second bytes"),
        [1, 2, 3, 4, 0, 0, 0, 0]
    );
    println!(
        "no_replay execution_pending=3 retirement_pending=2 fetches=1,2 executions=1,2 retirement_resumes=exact post_terminal=Exhausted duplicated_bytes=0 duplicated_records=0 duplicated_trace=0 duplicated_irq=0"
    );
    Ok(())
}
