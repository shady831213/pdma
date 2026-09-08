#![allow(missing_docs)]

use pdma_model_host::DriverError;

use hardware_interface_runtime::SoftwareCpu;
use pdma_model_contract::semantic::{EventErrorKind, desc::CopyRequest};
use pdma_model_device::AsyncDevice;
use pdma_model_host::{
    AsyncDriver, adapter::abi::AsyncDeviceAccess, adapter::abi::PdmaRegisterPort,
    compat::AsyncDevicePort,
};

mod recovery;

#[tokio::test(flavor = "current_thread")]
async fn async_runner_publishes_event_and_no_completion_when_linear_copy_reads_past_source() {
    // Given: async runner handle, AsyncDriver, and a linear copy whose source read is too long.
    let (runner, worker) = AsyncDevice::new(
        pdma_model_device::PdmaMemoryProvisioning::shared(std::sync::Arc::new(
            hardware_interface_runtime::SharedMemory::new(),
        )),
        pdma_model_contract::semantic::ResourceConfig::default(),
    );
    let worker = tokio::spawn(worker.run());
    let (snapshot, event, queue_id, command_id, src, no_completion, no_event) = SoftwareCpu::new()
        .run_main(async {
            let mut handle = PdmaRegisterPort::new(AsyncDeviceAccess::new(
                runner.handle(),
                pdma_model_host::adapter::abi::PdmaIrqCoordinator::default(),
            ))
            .expect("active CPU creates register port");
            let mut driver = AsyncDriver::setup_on(&mut handle, 4, 4)
                .await
                .expect("queue setup is valid");
            let src = driver
                .allocate_buffer_on(&mut handle, 4)
                .await
                .expect("allocation");
            let dst = driver
                .allocate_buffer_on(&mut handle, 8)
                .await
                .expect("allocation");

            // When: the async runner executes the invalid copy and software polls its event.
            let command_id = driver
                .submit_copy_on(&mut handle, CopyRequest::new(src, dst, 8))
                .await
                .expect("copy submission has ring space");
            let snapshot = runner.wait_for_error_after(0).await;
            let event = driver
                .poll_event_on(&mut handle)
                .await
                .expect("event ring exists")
                .expect("error event was published");
            let queue_id = driver.queue_id();
            let no_completion = driver.poll_completion_on(&mut handle).await;
            let no_event = driver.poll_event_on(&mut handle).await;
            (
                snapshot,
                event,
                queue_id,
                command_id,
                src,
                no_completion,
                no_event,
            )
        })
        .await;
    runner.request_shutdown().await;
    let summary = worker.await.expect("runner joins cleanly");

    // Then: the original out-of-bounds event appears and no success completion appears.
    assert_eq!(snapshot.errors, 1);
    assert_eq!(event.queue_id(), queue_id);
    assert_eq!(event.command_id(), command_id);
    assert_eq!(event.buffer(), src);
    assert_eq!(
        event.error_kind(),
        EventErrorKind::OutOfBounds { len: 4, end: 8 }
    );
    assert_eq!(no_completion, Ok(None));
    assert_eq!(no_event, Ok(None));
    assert_eq!(summary.errors, 1);
}

#[tokio::test(flavor = "current_thread")]
async fn async_runner_reclaims_driver_command_slot_when_error_event_is_polled() {
    // Given: async runner handle, AsyncDriver, and a one-slot queue with one invalid command.
    let (runner, worker) = AsyncDevice::new(
        pdma_model_device::PdmaMemoryProvisioning::shared(std::sync::Arc::new(
            hardware_interface_runtime::SharedMemory::new(),
        )),
        pdma_model_contract::semantic::ResourceConfig::default(),
    );
    let worker = tokio::spawn(worker.run());
    let (second_before, snapshot, event, queue_id, submit_after_ok) = SoftwareCpu::new()
        .run_main(async {
            let mut handle = PdmaRegisterPort::new(AsyncDeviceAccess::new(
                runner.handle(),
                pdma_model_host::adapter::abi::PdmaIrqCoordinator::default(),
            ))
            .expect("active CPU creates register port");
            let mut driver = AsyncDriver::setup_on(&mut handle, 1, 4)
                .await
                .expect("queue setup is valid");
            let src = driver
                .allocate_buffer_on(&mut handle, 4)
                .await
                .expect("allocation");
            let dst = driver
                .allocate_buffer_on(&mut handle, 8)
                .await
                .expect("allocation");
            driver
                .submit_copy_on(&mut handle, CopyRequest::new(src, dst, 8))
                .await
                .expect("first command has ring space");
            let second_before = driver
                .submit_copy_on(&mut handle, CopyRequest::new(src, dst, 1))
                .await;

            // When: the invalid command fails and the async driver polls its error event.
            let snapshot = runner.wait_for_error_after(0).await;
            let event = driver
                .poll_event_on(&mut handle)
                .await
                .expect("event ring exists")
                .expect("error event was published");
            let queue_id = driver.queue_id();
            let submit_after_ok = driver
                .submit_copy_on(&mut handle, CopyRequest::new(src, dst, 1))
                .await
                .is_ok();
            (second_before, snapshot, event, queue_id, submit_after_ok)
        })
        .await;
    runner.request_shutdown().await;
    let summary = worker.await.expect("runner joins cleanly");

    // Then: polling the event reclaims the command slot for this driver.
    assert_eq!(second_before, Err(DriverError::QueueFull));
    assert_eq!(snapshot.errors, 1);
    assert_eq!(event.queue_id(), queue_id);
    assert!(submit_after_ok);
    assert_eq!(summary.errors, 1);
}

#[tokio::test(flavor = "current_thread")]
async fn async_driver_event_polling_does_not_consume_another_queue_event() {
    // Given: one async driver's queue has a pending memory fault event.
    let (runner, worker) = AsyncDevice::new(
        pdma_model_device::PdmaMemoryProvisioning::shared(std::sync::Arc::new(
            hardware_interface_runtime::SharedMemory::new(),
        )),
        pdma_model_contract::semantic::ResourceConfig::default(),
    );
    let worker = tokio::spawn(worker.run());
    let (snapshot, wrong_event, owner_event, owner_queue_id, first_submit_ok, second_submit) =
        SoftwareCpu::new()
            .run_main(async {
                let mut handle = AsyncDevicePort::new(runner.handle());
                let mut driver_a = AsyncDriver::setup_on(&mut handle, 1, 4)
                    .await
                    .expect("queue setup is valid");
                let mut driver_b = AsyncDriver::setup_on(&mut handle, 1, 4)
                    .await
                    .expect("queue setup is valid");
                let src = driver_a
                    .allocate_buffer_on(&mut handle, 4)
                    .await
                    .expect("allocation");
                let dst = driver_a
                    .allocate_buffer_on(&mut handle, 8)
                    .await
                    .expect("allocation");
                let b_src = driver_b
                    .allocate_buffer_on(&mut handle, 1)
                    .await
                    .expect("allocation");
                let b_dst = driver_b
                    .allocate_buffer_on(&mut handle, 1)
                    .await
                    .expect("allocation");
                driver_a
                    .submit_copy_on(&mut handle, CopyRequest::new(src, dst, 8))
                    .await
                    .expect("first driver command has ring space");
                let snapshot = runner.wait_for_error_after(0).await;

                // When: a different async driver polls the queue-local event path.
                let wrong_driver_event = driver_b
                    .poll_event_on(&mut handle)
                    .await
                    .expect("event ring exists");
                let owner_event = driver_a
                    .poll_event_on(&mut handle)
                    .await
                    .expect("event ring exists")
                    .expect("owning driver can still poll its event");
                let owner_queue_id = driver_a.queue_id();
                let first_submit_ok = driver_b
                    .submit_copy_on(&mut handle, CopyRequest::new(b_src, b_dst, 1))
                    .await
                    .is_ok();
                let second_submit = driver_b
                    .submit_copy_on(&mut handle, CopyRequest::new(b_src, b_dst, 1))
                    .await;
                (
                    snapshot,
                    wrong_driver_event,
                    owner_event,
                    owner_queue_id,
                    first_submit_ok,
                    second_submit,
                )
            })
            .await;
    runner.request_shutdown().await;
    let summary = worker.await.expect("runner joins cleanly");

    // Then: the non-owning driver neither consumes the event nor reclaims its slot.
    assert_eq!(snapshot.errors, 1);
    assert_eq!(wrong_event, None);
    assert_eq!(owner_event.queue_id(), owner_queue_id);
    assert!(first_submit_ok);
    assert_eq!(second_submit, Err(DriverError::QueueFull));
    assert_eq!(summary.errors, 1);
}
