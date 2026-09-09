use super::register_access::{GatedAccess, drop_pending};
use hardware_interface_runtime::{SharedMemory, SoftwareCpu};
use pdma_model_contract::{
    abi::registers::{QueueRegisters, queue_offset},
    semantic::{CommandId, ResourceConfig},
};
use pdma_model_device::AsyncDevice;
use pdma_model_host::{
    AsyncDriver, CopyRequest,
    adapter::abi::{AsyncDeviceAccess, PdmaRegisterPort},
};
use std::sync::Arc;

#[expect(
    clippy::expect_used,
    reason = "the shared completion/event test scenario must fail on unexpected setup or retirement errors"
)]
async fn cancellation_preserves_record(fault: bool) {
    // Given: a real retirement record with its consumer publication gated.
    let storage = Arc::new(SharedMemory::new());
    let memory = pdma_model_host::adapter::abi::PdmaHostMemory::new(storage.clone());
    let (device, worker) = AsyncDevice::new(memory.memory_access(), ResourceConfig::default());
    let worker = tokio::spawn(worker.run());
    let access = GatedAccess::new(AsyncDeviceAccess::new(device.handle(), memory.clone()));
    SoftwareCpu::new()
        .run_main(async {
            let mut port = PdmaRegisterPort::new(access.clone()).expect("port");
            let mut driver = AsyncDriver::setup_on(&mut port, 1, 1)
                .await
                .expect("driver");
            let buffer = driver
                .allocate_buffer_on(&mut port, 1)
                .await
                .expect("buffer");
            driver
                .submit_copy_on(
                    &mut port,
                    CopyRequest::new(buffer, buffer, if fault { 2 } else { 1 }),
                )
                .await
                .expect("submit");
            if fault {
                device.wait_for_error_after(0).await;
            } else {
                device.wait_for_progress_after(0).await;
            }
            let consumer = if fault {
                QueueRegisters::EQ_CONSUMER.offset()
            } else {
                QueueRegisters::CQ_CONSUMER.offset()
            };
            access.arm_write(queue_offset(0, consumer).expect("consumer"));

            // When: polling is canceled before the hardware consumer advances.
            if fault {
                drop_pending(driver.poll_event_on(&mut port)).await;
            } else {
                drop_pending(driver.poll_completion_on(&mut port)).await;
            }
            access.release();
            let mut clone = port.clone();
            let mut peer = driver.clone();

            // Then: a clone receives the record once and restores the shared driver credit.
            if fault {
                assert_eq!(
                    peer.poll_event_on(&mut clone)
                        .await
                        .expect("event")
                        .expect("record")
                        .command_id(),
                    CommandId::new(0)
                );
                assert!(
                    driver
                        .poll_event_on(&mut port)
                        .await
                        .expect("drained")
                        .is_none()
                );
            } else {
                assert_eq!(
                    peer.poll_completion_on(&mut clone)
                        .await
                        .expect("completion")
                        .expect("record")
                        .command_id(),
                    CommandId::new(0)
                );
                assert!(
                    driver
                        .poll_completion_on(&mut port)
                        .await
                        .expect("drained")
                        .is_none()
                );
            }
            assert_eq!(
                driver
                    .submit_copy_on(&mut port, CopyRequest::new(buffer, buffer, 1))
                    .await
                    .expect("reclaimed credit"),
                CommandId::new(1)
            );
        })
        .await;
    device.request_shutdown().await;
    worker.await.expect("worker");
}

#[tokio::test(flavor = "current_thread")]
async fn cancelled_completion_poll_preserves_record_and_credit() {
    cancellation_preserves_record(false).await;
}

#[tokio::test(flavor = "current_thread")]
async fn cancelled_event_poll_preserves_record_and_credit() {
    cancellation_preserves_record(true).await;
}
