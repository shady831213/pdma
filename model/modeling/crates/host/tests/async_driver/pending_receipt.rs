use super::pending_commands::support::{GatedAccess, drop_pending};
use hardware_interface_runtime::{SharedMemory, SoftwareCpu};
use pdma_model_contract::semantic::{CopyRequest, ResourceConfig};
use pdma_model_device::{AsyncDevice, DeviceCore};
use pdma_model_host::{
    AsyncDriver, DriverError,
    adapter::abi::{AsyncDeviceAccess, PdmaRegisterPort},
    compat::AsyncDevicePort,
};
use std::sync::Arc;

#[tokio::test(flavor = "current_thread")]
async fn accepted_receipt_survives_a_cancelled_response_bank_and_last_port_drop() {
    receipt_survives_last_port_drop(0x198).await;
}

#[tokio::test(flavor = "current_thread")]
async fn issued_receipt_survives_an_unread_result_and_last_port_drop() {
    receipt_survives_last_port_drop(0x16c).await;
}

#[expect(
    clippy::expect_used,
    reason = "test fixture assertions must fail the test"
)]
async fn receipt_survives_last_port_drop(response_offset: u32) {
    // Given: a real depth-one queue pauses during its publication response.
    let (device, _worker) = AsyncDevice::from_core(DeviceCore::new(
        Arc::new(SharedMemory::new()),
        ResourceConfig::default(),
    ));
    let handle = device.handle();
    let access = GatedAccess::new(AsyncDeviceAccess::new(handle.clone()));
    SoftwareCpu::new()
        .run_main(async {
            let mut port = PdmaRegisterPort::new(access.clone()).expect("port");
            let mut driver = AsyncDriver::setup_on(&mut port, 1, 1)
                .await
                .expect("driver");
            let src = driver
                .allocate_buffer_on(&mut port, 4)
                .await
                .expect("source");
            let dst = driver
                .allocate_buffer_on(&mut port, 4)
                .await
                .expect("target");
            let request = CopyRequest::new(src, dst, 4);
            access.arm_after_at(3, response_offset);
            // When: the response future and final port owner are dropped after command issuance.
            drop_pending(driver.submit_copy_on(&mut port, request)).await;
            assert!(access.is_blocked());
            drop(port);
            // Then: accounting rejects before invoking another port, preserving accepted credit.
            let mut probe = AsyncDevicePort::new(handle);
            assert_eq!(
                driver.submit_copy_on(&mut probe, request).await,
                Err(DriverError::QueueFull)
            );
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn rejected_receipt_refunds_after_cancelled_details_and_last_port_drop() {
    // Given: real hardware rejects publication because its expected cursor was corrupted.
    let (device, _worker) = AsyncDevice::from_core(DeviceCore::new(
        Arc::new(SharedMemory::new()),
        ResourceConfig::default(),
    ));
    let handle = device.handle();
    let access = GatedAccess::new(AsyncDeviceAccess::new(handle.clone()));
    SoftwareCpu::new()
        .run_main(async {
            let mut port = PdmaRegisterPort::new(access.clone()).expect("port");
            let mut driver = AsyncDriver::setup_on(&mut port, 1, 1)
                .await
                .expect("driver");
            let src = driver
                .allocate_buffer_on(&mut port, 4)
                .await
                .expect("source");
            let dst = driver
                .allocate_buffer_on(&mut port, 4)
                .await
                .expect("target");
            let request = CopyRequest::new(src, dst, 4);
            access.reject_publication_before_details();
            // When: QResult rejected the command but its detail read and final port are dropped.
            drop_pending(driver.submit_copy_on(&mut port, request)).await;
            assert!(access.is_blocked());
            drop(port);
            let mut probe = super::support::BlockingPort::new(handle);
            probe.fail_next_submit();
            // Then: returned driver credit permits the retry to reach the configured port failure.
            assert_eq!(
                driver.submit_copy_on(&mut probe, request).await,
                Err(DriverError::Port(pdma_model_host::PortError::RingFull))
            );
        })
        .await;
}
