#![allow(missing_docs)]

use pdma_model_contract::{ModelError, desc::CopyRequest, ids::CommandId};
use pdma_model_device::AsyncDevice;
use pdma_model_host::AsyncDriver;

#[tokio::test(flavor = "current_thread")]
async fn submit_copy_on_rejects_second_command_when_local_slot_is_full() {
    // Given: a queue-port AsyncDevice, handle, AsyncDriver::setup_on(&mut handle, 1, 4), src/dst buffers.
    let (runner, worker) = AsyncDevice::queue_port();
    let worker = tokio::spawn(worker.run());
    let mut handle = runner.handle();
    let mut driver = AsyncDriver::setup_on(&mut handle, 1, 4)
        .await
        .expect("queue setup is valid");
    let src = driver.allocate_buffer_on(&mut handle, 4).await;
    let dst = driver.allocate_buffer_on(&mut handle, 4).await;

    // When: first async copy submit succeeds, second submit before polling is attempted.
    let first = driver
        .submit_copy_on(&mut handle, CopyRequest::new(src, dst, 4))
        .await
        .expect("first submit has a free slot");
    let second = driver
        .submit_copy_on(&mut handle, CopyRequest::new(src, dst, 4))
        .await;
    runner.request_shutdown().await;
    let _summary = worker.await.expect("runner joins cleanly");

    // Then: second submit returns ModelError::DriverQueueFull.
    assert_eq!(first, CommandId::new(0));
    assert_eq!(second, Err(ModelError::DriverQueueFull));
}
