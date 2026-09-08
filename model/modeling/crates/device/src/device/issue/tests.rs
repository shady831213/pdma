use hardware_interface_pipeline::{Stage, StepOutcome};
use pdma_model_contract::semantic::{
    Command, CommandId, CommandOperation, DeviceRunState, QueueConfigHandle, ResourceConfig,
    TraceEventKind,
    desc::{CopyRequest, VectorAddRequest, VectorBufferRange},
    queue::{EventRouting, QueueConfig, SoftwareOwnerId},
};

use super::{FetchedWork, IssueUnit};
use crate::AsyncStallReason;
use crate::device::{DeviceCore, context::HardwareContext};

fn fixture() -> (DeviceCore, [QueueConfigHandle; 2]) {
    let mut provisioning = crate::runtime::PdmaMemoryProvisioning::shared(std::sync::Arc::new(
        hardware_interface_runtime::SharedMemory::new(),
    ));
    let mut core = DeviceCore::new(
        provisioning.memory_access(),
        ResourceConfig::default().with_read_slots(1),
    );
    core.enable_tracing();
    let config = QueueConfig::new(4, 4, EventRouting::QueueLocal, SoftwareOwnerId::new(0));
    let vector = core.create_queue(config).expect("vector queue");
    let copy = core.create_queue(config).expect("copy queue");
    let source = provisioning
        .allocate(&mut core.memory, 4)
        .expect("source")
        .object();
    let destination = provisioning
        .allocate(&mut core.memory, 4)
        .expect("destination")
        .object();
    for (queue, operation) in [
        (
            vector,
            CommandOperation::VectorAdd(VectorAddRequest::new(
                VectorBufferRange::new(source, 0),
                VectorBufferRange::new(source, 0),
                VectorBufferRange::new(destination, 0),
                4,
            )),
        ),
        (
            copy,
            CommandOperation::Copy(CopyRequest::new(source, destination, 4)),
        ),
    ] {
        core.submit_command(queue, Command::new(CommandId::new(7), operation))
            .expect("submission");
        core.ring_doorbell(queue).expect("doorbell");
    }
    (core, [vector, copy])
}

fn issue(
    core: &mut DeviceCore,
    state: &mut <IssueUnit as Stage<HardwareContext<'_>>>::State,
) -> StepOutcome<FetchedWork, crate::ModelError, AsyncStallReason> {
    core.issue.step(
        &mut HardwareContext {
            queues: &mut core.queues,
            runnable: &mut core.runnable,
            memory: &core.memory,
            irq: &mut core.irq,
            trace: &mut core.trace,
            fetches: &mut core.native_fetches,
            resources: core.execution.resources(),
        },
        state,
    )
}

#[test]
fn standalone_issue_admits_fitting_copy_past_starved_vector() {
    // Given: the runnable head requires two read slots, while the following copy fits one.
    let (mut core, [vector, copy]) = fixture();
    let mut state = IssueUnit::start(());

    // When: Issue alone performs its acceptance step.
    let result = issue(&mut core, &mut state);

    // Then: only the copy is fetched, and the vector remains first for a future attempt.
    let StepOutcome::Complete(Ok(FetchedWork::Command(command))) = result else {
        panic!("fitting command must be issued: {result:?}");
    };
    assert_eq!(command.route.queue, copy.queue_id());
    assert_eq!(core.native_fetches, 1);
    assert_eq!(core.runnable.current(), Some(vector.queue_id()));
    assert_eq!(core.runnable.runnable_len(), 1);
    assert_eq!(
        core.queues.semantic_cursors(vector.queue_id()),
        Ok(((0, 1), (0, 0)))
    );
    assert_eq!(
        core.queues.semantic_cursors(copy.queue_id()),
        Ok(((0, 1), (0, 0)))
    );
    assert_eq!(
        core.trace_events()
            .iter()
            .filter(|event| event.kind() == TraceEventKind::Scheduled)
            .count(),
        1
    );
    assert_eq!(
        core.trace_events()
            .iter()
            .filter(|event| event.kind() == TraceEventKind::ExecutionSucceeded)
            .count(),
        0
    );
}

#[test]
fn standalone_issue_all_starved_rotates_full_fifo_without_fetch_or_execution() {
    // Given: both queues are starved before Issue starts.
    let (mut core, queues) = fixture();
    core.execution =
        crate::device::execution::ExecutionUnit::new(ResourceConfig::default().with_read_slots(0));
    let before = core.trace_events().len();
    let mut state = IssueUnit::start(());

    // When: the same pending Issue request is retried repeatedly.
    for _ in 0..3 {
        assert!(matches!(
            issue(&mut core, &mut state),
            StepOutcome::Pending(AsyncStallReason::ExecutionResourcePressure)
        ));
    }

    // Then: a full rotation retains FIFO order, and each attempt emits exactly one stall.
    assert_eq!(core.native_fetches, 0);
    assert_eq!(core.runnable.runnable_len(), 2);
    assert_eq!(core.runnable.next(), Some(queues[0].queue_id()));
    assert_eq!(core.runnable.next(), Some(queues[1].queue_id()));
    for queue in queues {
        assert_eq!(
            core.queues.semantic_cursors(queue.queue_id()),
            Ok(((0, 1), (0, 0)))
        );
    }
    let events = core.trace_events();
    assert_eq!(events.len(), before + 3);
    assert!(
        events
            .iter()
            .skip(before)
            .all(|event| event.kind() == TraceEventKind::Stall
                && event.queue_id() == Some(queues[0].queue_id()))
    );
}

#[test]
fn repeated_run_state_queries_leave_admission_and_hardware_untouched() {
    // Given: a starved head followed by fitting work.
    let (mut core, queues) = fixture();
    let trace = core.trace.clone();
    let irq = core.irq_snapshot();

    // When: software observes status repeatedly without requesting work.
    for _ in 0..8 {
        assert_eq!(core.run_state(), DeviceRunState::Blocked);
    }

    // Then: observation neither schedules the fitting queue nor rotates the head.
    assert_eq!(core.native_fetches, 0);
    assert_eq!(core.trace, trace);
    assert_eq!(core.irq_snapshot(), irq);
    assert!(core.command_pipeline.is_empty());
    assert!(!core.retirement.is_pending());
    assert_eq!(core.runnable.runnable_len(), 2);
    assert_eq!(core.runnable.next(), Some(queues[0].queue_id()));
    assert_eq!(core.runnable.next(), Some(queues[1].queue_id()));
    for queue in queues {
        assert_eq!(
            core.queues.semantic_cursors(queue.queue_id()),
            Ok(((0, 1), (0, 0)))
        );
    }
}

#[test]
fn pending_issue_resumes_same_request_when_copy_resources_return() {
    // Given: one retained Issue request that cannot admit either runnable queue.
    let (mut core, [vector, copy]) = fixture();
    core.execution =
        crate::device::execution::ExecutionUnit::new(ResourceConfig::default().with_read_slots(0));
    let mut state = IssueUnit::start(());
    assert!(matches!(
        issue(&mut core, &mut state),
        StepOutcome::Pending(AsyncStallReason::ExecutionResourcePressure)
    ));

    // When: one read slot returns and the same Issue request advances.
    core.execution =
        crate::device::execution::ExecutionUnit::new(ResourceConfig::default().with_read_slots(1));
    let result = issue(&mut core, &mut state);

    // Then: the fitting copy is fetched once and the hungry vector remains runnable.
    let StepOutcome::Complete(Ok(FetchedWork::Command(command))) = result else {
        panic!("pending Issue must resume: {result:?}");
    };
    assert_eq!(command.route.queue, copy.queue_id());
    assert_eq!(core.native_fetches, 1);
    assert_eq!(core.runnable.current(), Some(vector.queue_id()));
    assert_eq!(core.runnable.runnable_len(), 1);
    assert!(matches!(
        issue(&mut core, &mut state),
        StepOutcome::Exhausted
    ));
    assert_eq!(core.native_fetches, 1);
}
