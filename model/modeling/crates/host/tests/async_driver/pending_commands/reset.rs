use super::{GatedAccess, drop_pending};
use hardware_interface_runtime::{SharedMemory, SoftwareCpu};
use pdma_model_contract::semantic::{
    DeviceEpoch, EventRouting, QueueConfig, ResourceConfig, SoftwareOwnerId, WholeDeviceReset,
};
use pdma_model_device::AsyncDevice;
use pdma_model_host::{
    DriverPort, PortError,
    adapter::abi::{AsyncDeviceAccess, PdmaRegisterPort},
};
use std::sync::Arc;

#[tokio::test(flavor = "current_thread")]
async fn reset_discards_an_unread_activation_result_without_replaying_it() {
    // Given: ACTIVATE committed, but its response has not reached software.
    let (device, _worker) = AsyncDevice::new(
        pdma_model_device::PdmaMemoryProvisioning::shared(Arc::new(SharedMemory::new())),
        ResourceConfig::default(),
    );
    let handle = device.handle();
    let access = GatedAccess::new(AsyncDeviceAccess::new(
        handle.clone(),
        pdma_model_host::adapter::abi::PdmaIrqCoordinator::default(),
    ));
    SoftwareCpu::new()
        .run_main(async {
            let mut port = PdmaRegisterPort::new(access.clone()).expect("port");
            let config = QueueConfig::new(1, 1, EventRouting::QueueLocal, SoftwareOwnerId::new(0));
            access.arm_after(1);
            drop_pending(port.create_queue(config)).await;
            assert!(access.is_blocked());
            // When: hardware resets before software can recover the issued command result.
            let epoch = handle
                .reset(WholeDeviceReset::new(DeviceEpoch::initial()))
                .await
                .expect("reset");
            access.release();
            // Then: the lost response is not mistaken for acceptance; new epoch bootstrap is explicit.
            assert_eq!(
                port.create_queue(config).await,
                Err(PortError::SubmissionState)
            );
            assert_eq!(handle.memory_generation().await, 3);
            let mut fresh =
                PdmaRegisterPort::new_at(access.clone(), epoch).expect("new epoch port");
            let queue = fresh
                .create_queue(config)
                .await
                .expect("new epoch bootstrap");
            assert_eq!(queue.epoch(), epoch);
            assert_eq!(queue.queue_id().raw(), 0);
            assert_eq!(handle.memory_generation().await, 6);
        })
        .await;
}
