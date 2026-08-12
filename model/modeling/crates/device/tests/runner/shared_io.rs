use pdma_model_contract::{
    CommandId, CompletionStatus, ExternalCommand, QueueConfigHandle, ResourceConfig,
    TraceEventKind,
    desc::{CopyCommand, CopyRequest, VectorAddCommand, VectorAddRequest, VectorBufferRange},
    ids::BufferId,
    queue::{EventRouting, QueueConfig, SoftwareOwnerId},
};
use pdma_model_device::ModelError;
use pdma_model_device::{AsyncStallReason, AsyncStepStatus, SyncDevice};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

struct Fixture {
    device: SyncDevice,
    copy_queue: QueueConfigHandle,
    vector_queue: QueueConfigHandle,
    source: BufferId,
    second_destination: BufferId,
    vector_left: BufferId,
    vector_right: BufferId,
    vector_destination: BufferId,
}

pub fn assert_semantic_stall_commit_points() -> TestResult {
    let mut fixture = fixture()?;
    submit_commands(&mut fixture)?;
    verify_stall_commit_points(fixture)
}

fn fixture() -> TestResult<Fixture> {
    let mut device = SyncDevice::with_resource_config(
        ResourceConfig::default()
            .with_read_slots(1)
            .with_completion_publication_slots(1),
    );
    device.enable_tracing();
    let copy_queue = device.create_queue(config())?;
    let vector_queue = device.create_queue(config())?;
    let source = device.allocate_buffer(4)?;
    let second_destination = device.allocate_buffer(4)?;
    let vector_left = device.allocate_buffer(4)?;
    let vector_right = device.allocate_buffer(4)?;
    let vector_destination = device.allocate_buffer(4)?;
    device.write_buffer(source, 0, &[2, 4, 6, 8])?;
    Ok(Fixture {
        device,
        copy_queue,
        vector_queue,
        source,
        second_destination,
        vector_left,
        vector_right,
        vector_destination,
    })
}

fn submit_commands(fixture: &mut Fixture) -> TestResult {
    let first_destination = fixture.device.allocate_buffer(4)?;
    for (command_id, destination) in [
        (CommandId::new(0), first_destination),
        (CommandId::new(1), fixture.second_destination),
    ] {
        fixture.device.submit_command(
            fixture.copy_queue,
            ExternalCommand::Copy(CopyCommand::new(
                command_id,
                fixture.copy_queue.queue_id(),
                fixture.copy_queue.completion_ring_id(),
                CopyRequest::new(fixture.source, destination, 4),
            )),
        )?;
    }
    fixture.device.submit_command(
        fixture.vector_queue,
        ExternalCommand::VectorAdd(VectorAddCommand::new(
            CommandId::new(2),
            fixture.vector_queue.queue_id(),
            fixture.vector_queue.completion_ring_id(),
            VectorAddRequest::new(
                VectorBufferRange::new(fixture.vector_left, 0),
                VectorBufferRange::new(fixture.vector_right, 0),
                VectorBufferRange::new(fixture.vector_destination, 0),
                4,
            ),
        )),
    )?;
    fixture.device.ring_doorbell(fixture.copy_queue)?;
    fixture.device.ring_doorbell(fixture.vector_queue)?;
    Ok(())
}

fn verify_stall_commit_points(mut fixture: Fixture) -> TestResult {
    let first_stall = fixture.device.run_until_blocked(2)?;
    let publication_stall = fixture.device.run_until_blocked(1)?;
    let pending_retry = fixture.device.run_until_blocked(1)?;
    let first_completion = fixture.device.poll_completion(fixture.copy_queue)?;
    let retry_budget = fixture.device.run_until_blocked(1);
    let second_completion = fixture.device.poll_completion(fixture.copy_queue)?;
    let repeated_resource_stall = fixture.device.run_until_blocked(1)?;

    assert_stall(
        first_stall.last_status,
        AsyncStallReason::ExecutionResourcePressure,
    );
    assert_stall(
        publication_stall.last_status,
        AsyncStallReason::CompletionPublicationPressure,
    );
    assert_stall(
        pending_retry.last_status,
        AsyncStallReason::CompletionPublicationPressure,
    );
    assert_stall(
        repeated_resource_stall.last_status,
        AsyncStallReason::ExecutionResourcePressure,
    );
    assert_eq!(
        retry_budget,
        Err(ModelError::StepBudgetExhausted { max_steps: 1 })
    );
    assert_eq!(
        first_completion.map(pdma_model_contract::CompletionRecord::command_id),
        Some(CommandId::new(0))
    );
    assert_eq!(
        second_completion.map(|record| (record.command_id(), record.status())),
        Some((CommandId::new(1), CompletionStatus::Success))
    );
    assert_eq!(
        fixture.device.read_buffer(fixture.second_destination, 0, 4),
        Ok(vec![2, 4, 6, 8])
    );
    assert_trace_commit_points(&fixture.device);
    Ok(())
}

fn assert_stall(status: AsyncStepStatus, reason: AsyncStallReason) {
    assert_eq!(status, AsyncStepStatus::Stalled(reason));
}

fn assert_trace_commit_points(device: &SyncDevice) {
    assert_eq!(
        trace_sequences(
            device,
            TraceEventKind::ExecutionSucceeded,
            CommandId::new(1)
        )
        .len(),
        1
    );
    assert_eq!(
        trace_sequences(device, TraceEventKind::Scheduled, CommandId::new(1)).len(),
        1
    );
    let publication = trace_sequences(
        device,
        TraceEventKind::CompletionPublished,
        CommandId::new(1),
    )
    .into_iter()
    .next();
    let vector_schedules = trace_sequences(device, TraceEventKind::Scheduled, CommandId::new(2));
    assert_eq!(vector_schedules.len(), 2);
    assert_eq!(
        vector_schedules
            .iter()
            .filter(|sequence| Some(**sequence) < publication)
            .count(),
        1
    );
    assert_eq!(
        vector_schedules
            .iter()
            .filter(|sequence| Some(**sequence) > publication)
            .count(),
        1
    );
}

const fn config() -> QueueConfig {
    QueueConfig::new(4, 4, EventRouting::QueueLocal, SoftwareOwnerId::new(0))
}

fn trace_sequences(device: &SyncDevice, kind: TraceEventKind, command: CommandId) -> Vec<usize> {
    device
        .trace_events()
        .iter()
        .filter(|event| event.kind() == kind && event.command_id() == Some(command))
        .map(pdma_model_contract::TraceEvent::sequence)
        .collect()
}
