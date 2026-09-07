use hardware_interface_pipeline::{Operation, PipelineExt, StepOutcome};
use pdma_model_contract::semantic::{CommandId, CompletionRecord, CompletionStatus};

use super::support::prepared_pair;
use crate::HardwareStepStatus;
use crate::device::context::HardwareContext;
use crate::device::issue::IssueUnit;

#[test]
fn fetch_and_retirement_share_one_queue_state_in_composed_operators() {
    // Given: one device-owned queue with two commands and separate reusable operators.
    let mut prepared = prepared_pair();
    let core = &mut prepared.fixture.core;
    let mut context = HardwareContext {
        queues: &mut core.queues,
        runnable: &mut core.runnable,
        dma: &core.dma,
        irq: &mut core.irq,
        trace: &mut core.trace,
        fetches: &mut core.native_fetches,
        resources: core.execution.resources(),
    };
    let mut operators = IssueUnit::default().then((&mut core.execution).then(&mut core.retirement));

    // When: both requests run through fetch, execution, and retirement on that same state.
    for command in [CommandId::new(7), CommandId::new(99)] {
        let mut operation = Operation::new(());
        assert_eq!(
            operation.step(&mut operators, &mut context),
            StepOutcome::Progressed
        );
        assert_eq!(
            operation.step(&mut operators, &mut context),
            StepOutcome::Progressed
        );
        assert_eq!(
            operation.step(&mut operators, &mut context),
            StepOutcome::Complete(Ok(HardwareStepStatus::Progressed))
        );
        assert_eq!(
            operation.step(&mut operators, &mut context),
            StepOutcome::Exhausted
        );
        assert_eq!(
            context
                .queues
                .poll_semantic_completion(prepared.queue.completion_ring_id()),
            Ok(Some(CompletionRecord::new(
                prepared.queue.queue_id(),
                command,
                CompletionStatus::Success
            )))
        );
    }

    // Then: publication consumed both fetched commands, requeued later work, and latched IRQ.
    assert_eq!(*context.fetches, 2);
    assert_eq!(
        context.queues.semantic_cursors(prepared.queue.queue_id()),
        Ok(((2, 2), (2, 2)))
    );
    assert_eq!(context.runnable.current(), None);
    assert!(context.irq.line_asserted());
    for destination in prepared.destinations {
        assert_eq!(
            context.dma.read_object(destination.object(), 0, 8),
            Ok(vec![1, 2, 3, 4, 0, 0, 0, 0])
        );
    }
    println!(
        "SHARED_QUEUE|fetches=2|cursors=2/2/2/2|completions=7,99|irq=true|turns=3+3|same_queue_state=true"
    );
}
