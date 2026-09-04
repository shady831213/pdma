use super::super::{DeviceCore, ModelStepStatus};
use crate::AsyncStallReason;
use pdma_model_contract::semantic::{
    BufferId, Command, CommandId, CommandOperation, CompletionStatus, QueueConfigHandle,
    ResourceConfig, TraceEventKind,
    desc::{CopyRequest, VectorAddRequest, VectorBufferRange},
    queue::{EventRouting, QueueConfig, SoftwareOwnerId},
};

struct Fixture {
    core: DeviceCore,
    copy_queue: QueueConfigHandle,
    second_destination: BufferId,
}

fn fixture() -> Fixture {
    let mut core = DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        ResourceConfig::default()
            .with_read_slots(1)
            .with_completion_publication_slots(1),
    );
    core.enable_tracing();
    let config = QueueConfig::new(4, 4, EventRouting::QueueLocal, SoftwareOwnerId::new(0));
    let copy_queue = core.create_queue(config).expect("copy queue");
    let vector_queue = core.create_queue(config).expect("vector queue");
    let source = core.dma.allocate(4).expect("source").object();
    let first_destination = core.dma.allocate(4).expect("first destination").object();
    let second_destination = core.dma.allocate(4).expect("second destination").object();
    let vector_left = core.dma.allocate(4).expect("vector left").object();
    let vector_right = core.dma.allocate(4).expect("vector right").object();
    let vector_destination = core.dma.allocate(4).expect("vector destination").object();
    core.dma
        .write_object(source, 0, &[2, 4, 6, 8])
        .expect("source write");
    for (command, destination) in [first_destination, second_destination]
        .into_iter()
        .enumerate()
    {
        core.submit_command(
            copy_queue,
            Command::new(
                CommandId::new(command),
                CommandOperation::Copy(CopyRequest::new(source, destination, 4)),
            ),
        )
        .expect("copy submission");
    }
    core.submit_command(
        vector_queue,
        Command::new(
            CommandId::new(2),
            CommandOperation::VectorAdd(VectorAddRequest::new(
                VectorBufferRange::new(vector_left, 0),
                VectorBufferRange::new(vector_right, 0),
                VectorBufferRange::new(vector_destination, 0),
                4,
            )),
        ),
    )
    .expect("vector submission");
    core.ring_doorbell(copy_queue).expect("copy doorbell");
    core.ring_doorbell(vector_queue).expect("vector doorbell");
    Fixture {
        core,
        copy_queue,
        second_destination,
    }
}

#[test]
fn admission_skips_starved_work_without_replaying_accepted_retirement() {
    // Given: two copy commands and one vector command sharing constrained resources.
    let Fixture {
        mut core,
        copy_queue,
        second_destination,
    } = fixture();

    // When: the runnable copy reaches publication pressure before the starved vector is admitted.
    let first_copy = core.step();
    let publication_stall = core.step();
    let pending_retry = core.step();
    let first_completion = core.poll_completion(copy_queue).expect("completion ring");
    let retried = core.step();
    let second_completion = core.poll_completion(copy_queue).expect("completion ring");
    let resource_stall = core.step();
    let repeated_resource_stall = core.step();

    // Then: accepted retirement is retained while inadmissible work is never fetched or executed.
    assert_eq!(first_copy, Ok(ModelStepStatus::Progressed));
    assert_eq!(
        publication_stall,
        Ok(ModelStepStatus::Stalled(
            AsyncStallReason::CompletionPublicationPressure
        ))
    );
    assert_eq!(publication_stall, pending_retry);
    assert_eq!(retried, Ok(ModelStepStatus::Progressed));
    assert_eq!(
        resource_stall,
        Ok(ModelStepStatus::Stalled(
            AsyncStallReason::ExecutionResourcePressure
        ))
    );
    assert_eq!(repeated_resource_stall, resource_stall);
    assert_eq!(
        first_completion.map(pdma_model_contract::semantic::CompletionRecord::command_id),
        Some(CommandId::new(0))
    );
    assert_eq!(
        second_completion.map(|record| (record.command_id(), record.status())),
        Some((CommandId::new(1), CompletionStatus::Success))
    );
    assert_eq!(
        core.dma.read_object(second_destination, 0, 4),
        Ok(vec![2, 4, 6, 8])
    );
    let events = core.trace_events();
    assert_eq!(
        events
            .iter()
            .filter(|event| {
                event.kind() == TraceEventKind::ExecutionSucceeded
                    && event.command_id() == Some(CommandId::new(1))
            })
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| {
                event.kind() == TraceEventKind::Scheduled
                    && event.command_id() == Some(CommandId::new(2))
            })
            .count(),
        0
    );
}
