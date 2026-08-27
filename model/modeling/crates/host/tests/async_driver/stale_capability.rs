#[tokio::test(flavor = "current_thread")]
async fn stale_queue_and_result_capabilities_commit_nothing() {
    // Given: one real queue and a same-ID capability from a later device epoch.
    let mut device = SyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::semantic::ResourceConfig::default(),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    let queue = device
        .create_queue(QueueConfig::new(
            2,
            2,
            EventRouting::QueueLocal,
            SoftwareOwnerId::new(0),
        ))
        .expect("queue setup is valid");
    let stale_epoch = queue.epoch().next().expect("initial epoch can advance");
    let stale_queue = QueueConfigHandle::new(stale_epoch, queue.directory_entry());
    let source = device.allocate_buffer(1).expect("allocation");
    let destination = device.allocate_buffer(1).expect("allocation");
    let command = ExternalCommand::Copy(CopyCommand::new(
        CommandId::new(0),
        queue.queue_id(),
        queue.completion_ring_id(),
        CopyRequest::new(source, destination, 1),
    ));
    device.enable_tracing();
    let before_submit = device.trace_events();

    // When: stale queue capabilities reach submission and doorbell commit boundaries.
    let (stale_submit, stale_doorbell) = {
        let mut port = SemanticDevicePort::new(&mut device);
        let stale_submit =
            DriverPort::submit_command(&mut port, stale_queue, command.clone()).await;
        let stale_doorbell = DriverPort::ring_doorbell(&mut port, stale_queue).await;
        (stale_submit, stale_doorbell)
    };

    // Then: both fail with the typed epoch error and leave trace and queue cursors unchanged.
    assert_ne!(queue, stale_queue);
    assert_eq!(
        stale_submit,
        Err(PortError::StaleEpoch {
            expected: DeviceEpoch::initial(),
            actual: stale_epoch,
        })
    );
    assert_eq!(
        stale_doorbell,
        Err(PortError::StaleEpoch {
            expected: DeviceEpoch::initial(),
            actual: stale_epoch,
        })
    );
    assert_eq!(device.trace_events(), before_submit);
    {
        let mut port = SemanticDevicePort::new(&mut device);
        DriverPort::submit_command(&mut port, queue, command)
            .await
            .expect("current queue capability submits");
        DriverPort::ring_doorbell(&mut port, queue)
            .await
            .expect("current queue capability rings");
    }
    device
        .run_until_idle(1)
        .expect("current command retires exactly once");
    let before_result_poll = device.trace_events();
    let (stale_completion, stale_event) = {
        let mut port = SemanticDevicePort::new(&mut device);
        let stale_completion = DriverPort::poll_completion(&mut port, stale_queue).await;
        let stale_event = DriverPort::poll_queue_event(&mut port, stale_queue).await;
        (stale_completion, stale_event)
    };
    assert_eq!(
        stale_completion,
        Err(PortError::StaleEpoch {
            expected: DeviceEpoch::initial(),
            actual: stale_epoch,
        })
    );
    assert_eq!(
        stale_event,
        Err(PortError::StaleEpoch {
            expected: DeviceEpoch::initial(),
            actual: stale_epoch,
        })
    );
    assert_eq!(device.trace_events(), before_result_poll);
    let current_completion = {
        let mut port = SemanticDevicePort::new(&mut device);
        DriverPort::poll_completion(&mut port, queue)
            .await
            .expect("current result capability is valid")
    };
    assert!(current_completion.is_some());
    println!(
        "queue epochs current={} stale={} stale_submit={stale_submit:?} \
         stale_doorbell={stale_doorbell:?} stale_completion={stale_completion:?} \
         stale_event={stale_event:?} trace_unchanged=true record_retained=true",
        queue.epoch().raw(),
        stale_epoch.raw(),
    );
}
