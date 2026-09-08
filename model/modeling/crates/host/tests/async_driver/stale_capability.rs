use pdma_model_contract::semantic::{
    Command, CommandId, CommandOperation, DeviceEpoch, QueueConfigHandle,
    desc::CopyRequest,
    queue::{EventRouting, QueueConfig, SoftwareOwnerId},
};
use pdma_model_device::AsyncDevice;
use pdma_model_host::{DriverPort, PortError, compat::AsyncDevicePort};

#[tokio::test(flavor = "current_thread")]
async fn stale_queue_and_result_capabilities_commit_nothing() {
    // Given: one live queue and a same-ID capability from a later epoch.
    let (device, worker) = AsyncDevice::new(
        pdma_model_device::PdmaMemoryProvisioning::shared(std::sync::Arc::new(
            hardware_interface_runtime::SharedMemory::new(),
        )),
        pdma_model_contract::semantic::ResourceConfig::default(),
    );
    let handle = device.handle();
    let queue = handle
        .create_queue(QueueConfig::new(
            2,
            2,
            EventRouting::QueueLocal,
            SoftwareOwnerId::new(0),
        ))
        .await
        .expect("queue");
    let stale_epoch = queue.epoch().next().expect("epoch advances");
    let stale_queue = QueueConfigHandle::new(stale_epoch, queue.directory_entry());
    let source = handle.allocate_buffer(1).await.expect("source");
    let destination = handle.allocate_buffer(1).await.expect("destination");
    let command = Command::new(
        CommandId::new(0),
        CommandOperation::Copy(CopyRequest::new(source, destination, 1)),
    );
    handle.enable_tracing().await;
    let before_submit = handle.trace_events().await;
    let mut port = AsyncDevicePort::new(handle.clone());

    // When: stale capabilities reach submission, doorbell, and destructive result polls.
    let stale_submit = DriverPort::submit_command(&mut port, stale_queue, command.clone()).await;
    let stale_doorbell = DriverPort::ring_doorbell(&mut port, stale_queue).await;
    let stale_completion = DriverPort::poll_completion(&mut port, stale_queue).await;
    let stale_event = DriverPort::poll_queue_event(&mut port, stale_queue).await;

    // Then: typed epoch failures precede all trace, queue, and result mutation.
    for result in [
        stale_submit,
        stale_doorbell,
        stale_completion.map(|_| ()),
        stale_event.map(|_| ()),
    ] {
        assert_eq!(
            result,
            Err(PortError::StaleEpoch {
                expected: DeviceEpoch::initial(),
                actual: stale_epoch,
            })
        );
    }
    assert_eq!(handle.trace_events().await, before_submit);
    DriverPort::submit_command(&mut port, queue, command)
        .await
        .expect("current submission");
    DriverPort::ring_doorbell(&mut port, queue)
        .await
        .expect("current doorbell");
    let worker = tokio::spawn(worker.run());
    device.wait_for_progress_after(0).await;
    let current = DriverPort::poll_completion(&mut port, queue)
        .await
        .expect("current result");
    device.request_shutdown().await;
    worker.await.expect("worker joins");
    assert!(current.is_some());
}
