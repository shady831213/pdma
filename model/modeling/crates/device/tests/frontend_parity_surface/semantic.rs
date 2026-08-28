use crate::operations::OperationCase;
use pdma_model_contract::semantic::desc::{
    BroadcastCopyRequest, BroadcastDestinationRange, Command, CopyRequest, StridedCopyRequest,
    VectorAddRequest, VectorBufferRange,
};
use pdma_model_contract::semantic::{
    CommandId, DmaRangeStart, EventRouting, QueueConfig, ResourceConfig, SoftwareOwnerId,
    TraceEventKind, TraceOperation,
};
use pdma_model_device::SyncDevice;

type TestResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync + 'static>>;

pub(super) struct SemanticObservation {
    pub(super) destinations: Vec<Vec<u8>>,
    pub(super) traces: Vec<(TraceEventKind, Option<TraceOperation>)>,
    pub(super) executions: usize,
    pub(super) retirements: usize,
}

pub(super) fn observe(case: OperationCase) -> TestResult<SemanticObservation> {
    let mut device = SyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        ResourceConfig::default(),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    device.enable_tracing();
    let queue = device.create_queue(QueueConfig::new(
        2,
        2,
        EventRouting::QueueLocal,
        SoftwareOwnerId::new(0),
    ))?;
    let source_a = device.allocate_buffer(8)?;
    let source_b = device.allocate_buffer(8)?;
    let destination_a = device.allocate_buffer(8)?;
    let destination_b = device.allocate_buffer(8)?;
    device.write_buffer(source_a, 0, &[1, 2, 3, 4, 5, 6, 7, 8])?;
    device.write_buffer(source_b, 0, &[8, 7, 6, 5, 4, 3, 2, 1])?;
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
    device.submit_command(queue, command)?;
    device.ring_doorbell(queue)?;
    let summary = device.run_until_blocked(4)?;
    assert_eq!(summary.steps, 1);
    let retirement = device.poll_completion(queue)?;
    let retirements = usize::from(retirement.is_some());
    assert_eq!(retirements, 1);
    let destinations = vec![
        device.read_buffer(destination_a, 0, 8)?,
        device.read_buffer(destination_b, 0, 8)?,
    ];
    let executions = device.native_steps();
    let trace_events = device.trace_events();
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

pub(super) fn arbitration() -> TestResult<Vec<pdma_model_contract::semantic::QueueId>> {
    let mut device = SyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        ResourceConfig::default().with_compute_slots(0),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    device.enable_tracing();
    let config = QueueConfig::new(2, 2, EventRouting::QueueLocal, SoftwareOwnerId::new(0));
    let first = device.create_queue(config)?;
    let second = device.create_queue(config)?;
    let source = device.allocate_buffer(4)?;
    let source_b = device.allocate_buffer(4)?;
    let destination_a = device.allocate_buffer(4)?;
    let destination_b = device.allocate_buffer(4)?;
    device.write_buffer(source, 0, &[1, 2, 3, 4])?;
    device.write_buffer(source_b, 0, &[4, 3, 2, 1])?;
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
        device.submit_command(queue, command)?;
        device.ring_doorbell(queue)?;
    }
    let blocked = device.run_until_blocked(8)?;
    assert_eq!(blocked.stalls, 1);
    let progressed = device.run_until_blocked(8)?;
    assert_eq!(progressed.steps, 1);
    Ok(device
        .trace_events()
        .into_iter()
        .filter(|event| event.kind() == TraceEventKind::Scheduled)
        .filter_map(|event| event.queue_id())
        .collect())
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
