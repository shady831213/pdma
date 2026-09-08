mod reset;
pub mod support;

use hardware_interface_runtime::{SharedMemory, SoftwareCpu};
use pdma_model_contract::semantic::{
    CommandId, CopyRequest, EventRouting, QueueConfig, ResourceConfig, SoftwareOwnerId,
};
use pdma_model_device::{AsyncDevice, DeviceCore};
use pdma_model_host::{
    AsyncDriver, DriverError, DriverPort,
    adapter::abi::{AsyncDeviceAccess, PdmaRegisterPort},
};
use std::sync::Arc;
use support::{GatedAccess, drop_pending};

#[tokio::test(flavor = "current_thread")]
async fn cancelled_activation_response_is_recovered_by_a_clone_without_new_rings() {
    // Given: an empty real device and a gate after ACTIVATE has committed.
    let (device, _worker) = AsyncDevice::from_core(DeviceCore::new(
        Arc::new(SharedMemory::new()),
        ResourceConfig::default(),
    ));
    let access = GatedAccess::new(AsyncDeviceAccess::new(device.handle()));
    SoftwareCpu::new()
        .run_main(async {
            let mut port = PdmaRegisterPort::new(access.clone()).expect("port");
            let config = QueueConfig::new(1, 1, EventRouting::QueueLocal, SoftwareOwnerId::new(0));
            access.arm_after(1);
            // When: create is dropped while its already-issued command result is pending.
            drop_pending(port.create_queue(config)).await;
            assert!(access.is_blocked());
            access.release();
            let generation = device.handle().memory_generation().await;
            let queue = port
                .clone()
                .create_queue(config)
                .await
                .expect("recover activation");
            // Then: the same hardware queue is delivered; no second allocation/activation occurs.
            assert_eq!(queue.queue_id().raw(), 0);
            assert_eq!(device.handle().memory_generation().await, generation);
            assert_eq!(generation, 3);
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn cancelled_publication_response_keeps_the_accepted_driver_slot() {
    // Given: a depth-one queue with real storage and an explicit worker actor.
    let (device, worker) = AsyncDevice::from_core(DeviceCore::new(
        Arc::new(SharedMemory::new()),
        ResourceConfig::default(),
    ));
    let worker = tokio::spawn(worker.run());
    let access = GatedAccess::new(AsyncDeviceAccess::new(device.handle()));
    SoftwareCpu::new()
        .run_main(async {
            let mut port = PdmaRegisterPort::new(access.clone()).expect("port");
            let mut driver = AsyncDriver::setup_on(&mut port, 1, 1).await.expect("setup");
            let src = driver
                .allocate_buffer_on(&mut port, 4)
                .await
                .expect("source");
            let dst = driver
                .allocate_buffer_on(&mut port, 4)
                .await
                .expect("target");
            driver
                .write_buffer_on(&mut port, src, &[7, 6, 5, 4])
                .await
                .expect("payload");
            let request = CopyRequest::new(src, dst, 4);
            access.arm_after(3);
            // When: publication succeeded in hardware but the software response is canceled.
            drop_pending(driver.submit_copy_on(&mut port, request)).await;
            assert!(access.is_blocked());
            access.release();
            let mut clone = driver.clone();
            assert_eq!(
                clone.submit_copy_on(&mut port, request).await,
                Err(DriverError::QueueFull)
            );
            // Then: the accepted descriptor remains and can be explicitly doorbelled and retired once.
            port.ring_doorbell(driver.queue_config_handle())
                .await
                .expect("explicit doorbell");
            device.wait_for_progress_after(0).await;
            let completion = driver
                .poll_completion_on(&mut port)
                .await
                .expect("completion")
                .expect("record");
            assert_eq!(completion.command_id(), CommandId::new(0));
            assert_eq!(
                driver.read_buffer_on(&port, dst, 4).await.expect("copied"),
                [7, 6, 5, 4]
            );
            assert_eq!(
                clone
                    .submit_copy_on(&mut port, request)
                    .await
                    .expect("credit returned"),
                CommandId::new(1)
            );
        })
        .await;
    device.request_shutdown().await;
    worker.await.expect("worker joined");
}

#[tokio::test(flavor = "current_thread")]
async fn cancelled_completion_response_delivers_the_consumed_record_once() {
    cancelled_retirement_response(false).await;
}

#[tokio::test(flavor = "current_thread")]
async fn cancelled_event_response_delivers_the_consumed_record_once() {
    cancelled_retirement_response(true).await;
}

#[expect(
    clippy::expect_used,
    reason = "test fixture assertions must fail the test"
)]
async fn cancelled_retirement_response(fault: bool) {
    // Given: a completed command (or fault) in a real depth-one hardware queue.
    let (device, worker) = AsyncDevice::from_core(DeviceCore::new(
        Arc::new(SharedMemory::new()),
        ResourceConfig::default(),
    ));
    let worker = tokio::spawn(worker.run());
    let access = GatedAccess::new(AsyncDeviceAccess::new(device.handle()));
    SoftwareCpu::new()
        .run_main(async {
            let mut port = PdmaRegisterPort::new(access.clone()).expect("port");
            let mut driver = AsyncDriver::setup_on(&mut port, 1, 1).await.expect("setup");
            let src = driver
                .allocate_buffer_on(&mut port, 4)
                .await
                .expect("source");
            let dst = driver
                .allocate_buffer_on(&mut port, 4)
                .await
                .expect("target");
            let request = CopyRequest::new(src, dst, if fault { 5 } else { 4 });
            let command = driver
                .submit_copy_on(&mut port, request)
                .await
                .expect("submit");
            if fault {
                device.wait_for_error_after(0).await;
            } else {
                device.wait_for_progress_after(0).await;
            }
            access.arm_after(if fault { 9 } else { 7 });
            // When: the consumer write commits but its result read is canceled.
            if fault {
                drop_pending(driver.poll_event_on(&mut port)).await;
            } else {
                drop_pending(driver.poll_completion_on(&mut port)).await;
            }
            assert!(access.is_blocked());
            access.release();
            let mut clone = port.clone();
            // Then: a clone recovers the removed record exactly once and returns the slot credit.
            if fault {
                assert_eq!(
                    driver
                        .poll_event_on(&mut clone)
                        .await
                        .expect("recovered event")
                        .expect("event")
                        .command_id(),
                    command
                );
                assert!(
                    driver
                        .poll_event_on(&mut port)
                        .await
                        .expect("event empty")
                        .is_none()
                );
                assert!(
                    driver
                        .poll_completion_on(&mut port)
                        .await
                        .expect("no false success")
                        .is_none()
                );
            } else {
                assert_eq!(
                    driver
                        .poll_completion_on(&mut clone)
                        .await
                        .expect("recovered completion")
                        .expect("completion")
                        .command_id(),
                    command
                );
                assert!(
                    driver
                        .poll_completion_on(&mut port)
                        .await
                        .expect("completion empty")
                        .is_none()
                );
            }
            assert_eq!(
                driver
                    .submit_copy_on(&mut port, CopyRequest::new(src, dst, 4))
                    .await
                    .expect("credit returned"),
                CommandId::new(1)
            );
        })
        .await;
    device.request_shutdown().await;
    worker.await.expect("worker joined");
}
