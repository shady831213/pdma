use pdma_model_contract::{
    CommandId, ExternalCommand, ModelError, ResourceConfig, TraceEventKind,
    desc::{CopyCommand, CopyRequest},
    queue::{EventRouting, QueueConfig, SoftwareOwnerId},
};
use pdma_model_device::{AsyncStallReason, AsyncStepStatus, SyncDevice};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// Exercises a decoded DMA-bus command through the shared execution core.
///
/// # Panics
///
/// Panics when the observed queue, memory, publication, or trace behavior
/// differs from the expected DMA-bus frontend contract.
pub fn assert_dma_bus_decoded_command_uses_shared_core() -> TestResult {
    let mut device = SyncDevice::with_resource_config(
        ResourceConfig::default().with_completion_publication_slots(1),
    );
    device.enable_tracing();
    let queue = device.create_dma_bus_queue(QueueConfig::new(
        4,
        4,
        EventRouting::QueueLocal,
        SoftwareOwnerId::new(0),
    ))?;
    let semantic_queue = device.create_queue(QueueConfig::new(
        4,
        4,
        EventRouting::QueueLocal,
        SoftwareOwnerId::new(0),
    ))?;
    let source = device.allocate_buffer(4)?;
    let destination = device.allocate_buffer(4)?;
    device.write_buffer(source, 0, &[1, 3, 5, 7])?;
    let command = ExternalCommand::Copy(CopyCommand::new(
        CommandId::new(7),
        queue.queue_id(),
        queue.completion_ring_id(),
        CopyRequest::new(source, destination, 4),
    ));
    let wrong_frontend = device.submit_decoded_dma_command(semantic_queue, command.clone());
    assert_eq!(
        wrong_frontend,
        Err(ModelError::UnknownQueue(semantic_queue.queue_id()))
    );
    device.submit_decoded_dma_command(queue, command.clone())?;
    let occupied_slot = device.submit_decoded_dma_command(queue, command);
    assert_eq!(occupied_slot, Err(ModelError::RingFull));
    device.ring_doorbell(queue)?;

    let first = device.run_until_blocked(1)?;
    let retry = device.run_until_blocked(1)?;

    assert_eq!(
        first.last_status,
        AsyncStepStatus::Stalled(AsyncStallReason::CompletionPublicationPressure)
    );
    assert_eq!(
        retry.last_status,
        AsyncStepStatus::Stalled(AsyncStallReason::CompletionPublicationPressure)
    );
    assert_eq!(device.read_buffer(destination, 0, 4), Ok(vec![1, 3, 5, 7]));
    assert_eq!(device.poll_completion(queue), Ok(None));
    assert_eq!(
        trace_count(&device, TraceEventKind::Scheduled, CommandId::new(7)),
        1
    );
    assert_eq!(
        trace_count(
            &device,
            TraceEventKind::ExecutionSucceeded,
            CommandId::new(7)
        ),
        1
    );
    assert_eq!(
        trace_count(
            &device,
            TraceEventKind::CompletionPublished,
            CommandId::new(7)
        ),
        0
    );
    println!(
        "wrong_frontend={wrong_frontend:?} occupied_slot={occupied_slot:?} \
         destination={:?} first={:?} retry={:?}",
        device.read_buffer(destination, 0, 4),
        first.last_status,
        retry.last_status
    );
    Ok(())
}

fn trace_count(device: &SyncDevice, kind: TraceEventKind, command: CommandId) -> usize {
    device
        .trace_events()
        .iter()
        .filter(|event| event.kind() == kind && event.command_id() == Some(command))
        .count()
}
