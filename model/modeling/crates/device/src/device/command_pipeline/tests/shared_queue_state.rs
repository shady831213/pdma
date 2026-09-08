use hardware_interface_pipeline::{AfterEach, Operation, PipelineExt, StepOutcome};
use pdma_model_contract::abi::AbiCompletion;

use super::support::prepared_pair;
use crate::HardwareStepStatus;
use crate::device::context::HardwareContext;
use crate::device::issue::IssueUnit;

#[test]
fn fetch_and_retirement_share_one_queue_state_in_composed_operators() {
    // Given: one device-owned queue with two commands and separate reusable operators.
    let mut prepared = prepared_pair();
    let core = &mut prepared.fixture.core;
    // When: both requests run through fetch, execution, and retirement on that same state.
    for command in [7, 99] {
        let mut context = HardwareContext {
            queues: &mut core.queues,
            runnable: &mut core.runnable,
            memory: &core.memory,
            trace: &mut core.trace,
            fetches: &mut core.native_fetches,
            resources: core.command_pipeline.execution.resources(),
        };
        let mut operators = AfterEach::new(
            IssueUnit::default().then(
                (&mut core.command_pipeline.execution).then(&mut core.command_pipeline.retirement),
            ),
            &mut core.irq,
        );
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
            crate::test_support::poll_completion(core, prepared.queue),
            Some(AbiCompletion::success(
                u64::from(prepared.queue.index),
                command
            ))
        );
    }

    // Then: publication consumed both fetched commands, requeued later work, and latched IRQ.
    assert_eq!(core.native_fetches, 2);
    assert_eq!(
        super::support::cursors(core, prepared.queue),
        ((2, 2), (2, 2))
    );
    assert_eq!(core.runnable.current(), None);
    assert!(core.irq_snapshot().expect("line source").asserted());
    for destination in prepared.destinations {
        assert_eq!(
            core.memory.read_object(destination.object(), 0, 8),
            Ok(vec![1, 2, 3, 4, 0, 0, 0, 0])
        );
    }
    println!(
        "SHARED_QUEUE|fetches=2|cursors=2/2/2/2|completions=7,99|irq=true|turns=3+3|same_queue_state=true"
    );
}
