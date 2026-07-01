#![allow(missing_docs)]

use pdma_model_contract::{
    EventRouting, ModelError, QueueConfig, SoftwareOwnerId, desc::CopyRequest,
};
use pdma_model_device::PdmaDevice;
use pdma_model_device_async::AsyncDeviceRunner;
use pdma_model_host_async::AsyncDriver;

#[tokio::test(flavor = "current_thread")]
async fn async_driver_enforces_command_depth_from_explicit_queue_config() {
    // Given: an async driver created from an explicit one-slot queue config.
    let device = PdmaDevice::new();
    let runner = AsyncDeviceRunner::spawn(device);
    let mut handle = runner.handle();
    let mut driver = AsyncDriver::setup_with_config_on(
        &mut handle,
        QueueConfig::new(1, 2, EventRouting::QueueLocal, SoftwareOwnerId::new(7)),
    )
    .await
    .expect("queue config is valid");
    let src = driver.allocate_buffer_on(&mut handle, 1).await;
    let dst = driver.allocate_buffer_on(&mut handle, 1).await;

    // When: software submits one command and tries to submit another before reclaim.
    let command_id = driver
        .submit_copy_on(&mut handle, CopyRequest::new(src, dst, 1))
        .await
        .expect("first command has ring space");
    let second_submit = driver
        .submit_copy_on(&mut handle, CopyRequest::new(src, dst, 1))
        .await;
    runner.request_shutdown().await;
    runner.join().await.expect("runner joins cleanly");

    // Then: the explicit command depth is enforced and the handle records ownership.
    assert_eq!(command_id.raw(), 0);
    assert_eq!(second_submit, Err(ModelError::DriverQueueFull));
    assert_eq!(driver.queue_config_handle().command_depth(), 1);
    assert_eq!(driver.queue_config_handle().completion_depth(), 2);
    assert_eq!(
        driver.queue_config_handle().software_owner(),
        SoftwareOwnerId::new(7)
    );
}
