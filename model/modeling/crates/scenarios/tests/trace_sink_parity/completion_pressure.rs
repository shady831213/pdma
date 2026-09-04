use hardware_interface_runtime::SoftwareCpu;
use pdma_model_contract::semantic::{
    ResourceConfig, TraceEvent, TraceEventKind,
    desc::{CopyRequest, VectorAddRequest, VectorBufferRange},
    ids::{CommandId, QueueId},
};
use pdma_model_device::{AsyncDevice, AsyncStallReason, AsyncStepStatus};
use pdma_model_host::{AsyncDriver, compat::AsyncDevicePort};

#[allow(clippy::redundant_pub_crate)]
pub(super) struct CompletionPressureTrace {
    pub(super) events: Vec<TraceEvent>,
    pub(super) second_command: CommandId,
}

#[allow(clippy::expect_used, clippy::redundant_pub_crate)]
pub(super) async fn completion_pressure_trace() -> CompletionPressureTrace {
    let (runner, worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        ResourceConfig::default()
            .with_compute_slots(0)
            .with_completion_publication_slots(1),
    ));
    let worker = tokio::spawn(worker.run());
    let raw_handle = runner.handle();
    let handle = AsyncDevicePort::new(raw_handle.clone());
    raw_handle.enable_tracing().await;
    raw_handle.clear_trace().await;

    let (second_command, resource_command, resource_queue) =
        exercise_completion_pressure(&runner, handle).await;
    let events = raw_handle.trace_events().await;
    runner.request_shutdown().await;
    let _summary = worker.await.expect("runner joins cleanly");
    assert_resource_pre_admission(&events, resource_queue, resource_command);

    CompletionPressureTrace {
        events,
        second_command,
    }
}

#[allow(clippy::expect_used)]
async fn exercise_completion_pressure(
    runner: &AsyncDevice,
    mut handle: AsyncDevicePort,
) -> (CommandId, CommandId, QueueId) {
    SoftwareCpu::new()
        .run_main(async {
            let mut vector_driver = AsyncDriver::setup_on(&mut handle, 4, 4)
                .await
                .expect("queue setup is valid");
            let mut copy_driver = AsyncDriver::setup_on(&mut handle, 4, 4)
                .await
                .expect("queue setup is valid");
            let vector_src_a = vector_driver
                .allocate_buffer_on(&mut handle, 4)
                .await
                .expect("allocation");
            let vector_src_b = vector_driver
                .allocate_buffer_on(&mut handle, 4)
                .await
                .expect("allocation");
            let vector_dst = vector_driver
                .allocate_buffer_on(&mut handle, 4)
                .await
                .expect("allocation");
            let src_a = copy_driver
                .allocate_buffer_on(&mut handle, 4)
                .await
                .expect("allocation");
            let dst_a = copy_driver
                .allocate_buffer_on(&mut handle, 4)
                .await
                .expect("allocation");
            let src_b = copy_driver
                .allocate_buffer_on(&mut handle, 4)
                .await
                .expect("allocation");
            let dst_b = copy_driver
                .allocate_buffer_on(&mut handle, 4)
                .await
                .expect("allocation");
            copy_driver
                .write_buffer_on(&mut handle, src_a, &[1, 2, 3, 4])
                .await
                .expect("source A write is in bounds");
            copy_driver
                .write_buffer_on(&mut handle, src_b, &[5, 6, 7, 8])
                .await
                .expect("source B write is in bounds");
            let resource_command = vector_driver
                .submit_vector_add_on(
                    &mut handle,
                    VectorAddRequest::new(
                        VectorBufferRange::new(vector_src_a, 0),
                        VectorBufferRange::new(vector_src_b, 0),
                        VectorBufferRange::new(vector_dst, 0),
                        4,
                    ),
                )
                .await
                .expect("resource-starved vector submission has ring space");
            let _first_command = copy_driver
                .submit_copy_on(&mut handle, CopyRequest::new(src_a, dst_a, 4))
                .await
                .expect("first copy submission has ring space");
            let second_command = copy_driver
                .submit_copy_on(&mut handle, CopyRequest::new(src_b, dst_b, 4))
                .await
                .expect("second copy submission has ring space");
            let mut snapshot = runner.snapshot().await;
            while snapshot.last_status
                != AsyncStepStatus::Stalled(AsyncStallReason::CompletionPublicationPressure)
            {
                snapshot = runner.wait_for_change_after(snapshot).await;
            }
            let _first_completion = copy_driver
                .poll_completion_on(&mut handle)
                .await
                .expect("completion ring exists")
                .expect("first completion was published");
            let _resource_snapshot = runner.wait_for_stall_after(snapshot.stalls).await;
            (second_command, resource_command, vector_driver.queue_id())
        })
        .await
}

#[allow(clippy::expect_used)]
fn assert_resource_pre_admission(events: &[TraceEvent], queue: QueueId, command: CommandId) {
    let stall = events
        .iter()
        .position(|event| {
            event.kind() == TraceEventKind::Stall
                && event.detail() == "ExecutionResourcePressure"
                && event.queue_id() == Some(queue)
        })
        .expect("resource stall is traced");
    assert_eq!(events.get(stall).map(TraceEvent::command_id), Some(None));
    assert_eq!(
        event_position(events, TraceEventKind::Scheduled, queue, command),
        None
    );
    assert_eq!(
        event_position(events, TraceEventKind::ExecutionSucceeded, queue, command),
        None
    );
}

fn event_position(
    events: &[TraceEvent],
    kind: TraceEventKind,
    queue: QueueId,
    command: CommandId,
) -> Option<usize> {
    events.iter().position(|event| {
        event.kind() == kind
            && event.queue_id() == Some(queue)
            && event.command_id() == Some(command)
    })
}
