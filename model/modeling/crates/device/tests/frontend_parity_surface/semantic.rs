use crate::operations::OperationCase;
use pdma_model_contract::semantic::desc::{
    BroadcastCopyRequest, BroadcastDestinationRange, Command, CopyRequest, StridedCopyRequest,
    VectorAddRequest, VectorBufferRange,
};
use pdma_model_contract::semantic::{
    CommandId, DmaRangeStart, EventRouting, QueueConfig, ResourceConfig, SoftwareOwnerId,
    TraceEventKind, TraceOperation,
};
use pdma_model_device::AsyncDevice;

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync + 'static>>;

pub(super) struct SemanticObservation {
    pub(super) destinations: Vec<Vec<u8>>,
    pub(super) traces: Vec<(TraceEventKind, Option<TraceOperation>)>,
    pub(super) executions: usize,
    pub(super) retirements: usize,
}

pub(super) async fn observe(case: OperationCase) -> TestResult<SemanticObservation> {
    let (device, worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        ResourceConfig::default(),
    ));
    let worker = tokio::spawn(worker.run());
    let handle = device.handle();
    handle.enable_tracing().await;
    let queue = handle
        .create_queue(QueueConfig::new(
            2,
            2,
            EventRouting::QueueLocal,
            SoftwareOwnerId::new(0),
        ))
        .await?;
    let source_a = handle.allocate_buffer(8).await?;
    let source_b = handle.allocate_buffer(8).await?;
    let destination_a = handle.allocate_buffer(8).await?;
    let destination_b = handle.allocate_buffer(8).await?;
    handle
        .write_buffer(source_a, 0, &[1, 2, 3, 4, 5, 6, 7, 8])
        .await?;
    handle
        .write_buffer(source_b, 0, &[8, 7, 6, 5, 4, 3, 2, 1])
        .await?;
    let command_id = CommandId::new(match case {
        OperationCase::Copy => 10,
        OperationCase::Strided => 11,
        OperationCase::Broadcast => 12,
        OperationCase::Vector => 13,
    });
    let command = command(
        case,
        command_id,
        [source_a, source_b, destination_a, destination_b],
    );
    handle.submit_command(queue, command).await?;
    handle.ring_doorbell(queue).await?;
    let snapshot = device.wait_for_progress_after(0).await;
    assert_eq!(snapshot.steps, 1);
    let retirement = handle.poll_completion(queue).await?;
    let retirements = usize::from(retirement.is_some());
    assert_eq!(retirements, 1);
    let destinations = vec![
        handle.read_buffer(destination_a, 0, 8).await?,
        handle.read_buffer(destination_b, 0, 8).await?,
    ];
    let executions = handle.native_steps().await;
    let trace_events = handle.trace_events().await;
    device.request_shutdown().await;
    worker.await?;
    let traces = trace_events
        .into_iter()
        .filter(|event| {
            matches!(
                event.kind(),
                TraceEventKind::Scheduled | TraceEventKind::ExecutionSucceeded
            )
        })
        .map(|event| (event.kind(), event.operation()))
        .collect();
    Ok(SemanticObservation {
        destinations,
        traces,
        executions,
        retirements,
    })
}

pub(super) async fn arbitration() -> TestResult<Vec<pdma_model_contract::semantic::QueueId>> {
    let (device, worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        ResourceConfig::default().with_compute_slots(0),
    ));
    let worker = tokio::spawn(worker.run());
    let handle = device.handle();
    handle.enable_tracing().await;
    let config = QueueConfig::new(2, 2, EventRouting::QueueLocal, SoftwareOwnerId::new(0));
    let first = handle.create_queue(config).await?;
    let second = handle.create_queue(config).await?;
    let source = handle.allocate_buffer(4).await?;
    let source_b = handle.allocate_buffer(4).await?;
    let destination_a = handle.allocate_buffer(4).await?;
    let destination_b = handle.allocate_buffer(4).await?;
    handle.write_buffer(source, 0, &[1, 2, 3, 4]).await?;
    handle.write_buffer(source_b, 0, &[4, 3, 2, 1]).await?;
    let commands = [
        (
            first,
            Command::new(
                CommandId::new(40),
                pdma_model_contract::semantic::CommandOperation::VectorAdd(VectorAddRequest::new(
                    VectorBufferRange::new(source, 0),
                    VectorBufferRange::new(source_b, 0),
                    VectorBufferRange::new(destination_a, 0),
                    4,
                )),
            ),
        ),
        (
            second,
            Command::new(
                CommandId::new(41),
                pdma_model_contract::semantic::CommandOperation::Copy(CopyRequest::new(
                    source,
                    destination_b,
                    4,
                )),
            ),
        ),
    ];
    for (queue, command) in commands {
        handle.submit_command(queue, command).await?;
        handle.ring_doorbell(queue).await?;
    }
    let blocked = device.wait_for_stall_after(0).await;
    assert_eq!(blocked.stalls, 1);
    handle.ring_doorbell(second).await?;
    let progressed = device.wait_for_progress_after(0).await;
    assert_eq!(progressed.steps, 1);
    let queues = handle
        .trace_events()
        .await
        .into_iter()
        .filter(|event| event.kind() == TraceEventKind::Scheduled)
        .filter_map(|event| event.queue_id())
        .collect();
    device.request_shutdown().await;
    worker.await?;
    Ok(queues)
}

fn command(
    case: OperationCase,
    command_id: CommandId,
    buffers: [pdma_model_contract::semantic::BufferId; 4],
) -> Command {
    let [source_a, source_b, destination_a, destination_b] = buffers;
    match case {
        OperationCase::Copy => Command::new(
            command_id,
            pdma_model_contract::semantic::CommandOperation::Copy(CopyRequest::with_starts(
                DmaRangeStart::new(source_a, 2),
                DmaRangeStart::new(destination_a, 1),
                4,
            )),
        ),
        OperationCase::Strided => Command::new(
            command_id,
            pdma_model_contract::semantic::CommandOperation::StridedCopy(
                StridedCopyRequest::with_starts(
                    DmaRangeStart::new(source_a, 1),
                    DmaRangeStart::new(destination_a, 0),
                    2,
                    2,
                    3,
                    3,
                ),
            ),
        ),
        OperationCase::Broadcast => Command::new(
            command_id,
            pdma_model_contract::semantic::CommandOperation::BroadcastCopy(
                BroadcastCopyRequest::new(
                    source_a,
                    2,
                    3,
                    vec![
                        BroadcastDestinationRange::new(destination_a, 0),
                        BroadcastDestinationRange::new(destination_b, 0),
                    ],
                ),
            ),
        ),
        OperationCase::Vector => Command::new(
            command_id,
            pdma_model_contract::semantic::CommandOperation::VectorAdd(VectorAddRequest::new(
                VectorBufferRange::new(source_a, 1),
                VectorBufferRange::new(source_b, 1),
                VectorBufferRange::new(destination_a, 2),
                4,
            )),
        ),
    }
}
