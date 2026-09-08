use hardware_interface_runtime::{
    AccessOrigin, RegisterObservation, RegisterObserver, SharedMemory, SoftwareCpu,
};
use pdma_model_contract::semantic::{EventRouting, QueueConfig, ResourceConfig, SoftwareOwnerId};
use pdma_model_device::{AsyncDevice, PdmaMemoryProvisioning};
use pdma_model_host::{
    DriverPort,
    adapter::abi::{AsyncDeviceAccess, PdmaIrqCoordinator, PdmaRegisterPort},
};
use std::sync::{Arc, Mutex};

use super::pending_commands::support::{GatedAccess, drop_pending};
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment, RegisterAccess};
use pdma_model_contract::{
    abi::mmio::QueueCommand,
    semantic::{EpochQueue, QueueControl},
};

mod publication;

#[derive(Default)]
struct Observer(Mutex<Vec<RegisterObservation>>);

#[expect(
    clippy::expect_used,
    reason = "test fixture assertions must fail the test"
)]
impl RegisterObserver for Observer {
    fn observe(&self, observation: RegisterObservation) {
        self.0.lock().expect("observer lock").push(observation);
    }
}

#[tokio::test(flavor = "current_thread")]
async fn queue_bootstrap_uses_individual_register_accesses() {
    // Given: the canonical adapter shares real device memory and observed MMIO.
    let observer = Arc::new(Observer::default());
    let (device, _worker) = AsyncDevice::new_observed(
        PdmaMemoryProvisioning::shared(Arc::new(SharedMemory::new())),
        ResourceConfig::default(),
        observer.clone(),
    );
    let handle = device.handle();
    // When: software bootstraps its queue through the register port.
    let status = SoftwareCpu::new()
        .run_main(async {
            let mut port = PdmaRegisterPort::new(AsyncDeviceAccess::new(
                handle.clone(),
                PdmaIrqCoordinator::default(),
            ))
            .expect("port");
            let queue = port.create_queue(config()).await.expect("queue");
            port.queue_status(EpochQueue::new(queue.epoch(), queue.queue_id()))
                .await
                .expect("status")
        })
        .await;
    // Then: all three mappings persist and the empty queue was activated by direct accesses.
    assert_eq!(handle.memory_generation().await, 3);
    assert_eq!(status.submission().producer().raw(), 0);
    assert!(
        observer
            .0
            .lock()
            .expect("observer lock")
            .iter()
            .all(|item| item.origin == AccessOrigin::Direct)
    );
}

#[tokio::test(flavor = "current_thread")]
async fn canceled_activation_preserves_allocated_rings_and_staging() {
    // Given: activation pauses before its command write reaches the real device.
    let (device, _worker) = AsyncDevice::new(
        PdmaMemoryProvisioning::shared(Arc::new(SharedMemory::new())),
        ResourceConfig::default(),
    );
    let handle = device.handle();
    let access = GatedAccess::new(AsyncDeviceAccess::new(
        handle.clone(),
        PdmaIrqCoordinator::default(),
    ));
    SoftwareCpu::new()
        .run_main(async {
            let mut port = PdmaRegisterPort::new(access.clone()).expect("port");
            access.arm_before(QueueCommand::Activate.code());
            // When: software drops queue creation after its ring allocation and staging writes.
            drop_pending(port.create_queue(config())).await;
            assert!(access.is_blocked());
            // Then: each allocation remains, staged memory is mapped, and no queue was activated.
            assert_eq!(handle.memory_generation().await, 3);
            let mut base = [0; 8];
            RegisterAccess::read(&access, 0x138, &mut base)
                .await
                .expect("SQ base");
            let mut bytes = [1; 64];
            MemoryAccess::read(
                &access,
                MemoryAddress::new(u64::from_le_bytes(base)),
                MemoryAlignment::ONE,
                &mut bytes,
            )
            .expect("allocated ring");
            assert_eq!(bytes, [0; 64]);
            let queue = EpochQueue::new(
                pdma_model_contract::semantic::DeviceEpoch::initial(),
                pdma_model_contract::semantic::QueueId::new(0),
            );
            assert!(
                handle
                    .control_queue(QueueControl::ReadStatus(queue))
                    .await
                    .is_err()
            );
        })
        .await;
}

const fn config() -> QueueConfig {
    QueueConfig::new(2, 2, EventRouting::QueueLocal, SoftwareOwnerId::new(0))
}
