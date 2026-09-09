use hardware_interface_runtime::{
    AccessOrigin, RegisterObservation, RegisterObserver, SharedMemory, SoftwareCpu,
};
use pdma_model_contract::semantic::ResourceConfig;
use pdma_model_device::AsyncDevice;
use pdma_model_host::{
    DriverPort,
    adapter::abi::{AsyncDeviceAccess, PdmaRegisterPort},
};
use pdma_model_host::{EventRouting, QueueConfig, SoftwareOwnerId};
use std::sync::{Arc, Mutex};

use super::register_access::{GatedAccess, drop_pending};
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment, RegisterAccess};
use pdma_model_contract::{
    abi::registers::{QueueRegisters, queue_offset},
    semantic::EpochQueue,
};

mod irq_snapshot;
mod multiqueue;
mod publication;

#[tokio::test(flavor = "current_thread")]
async fn cloned_register_ports_create_distinct_queues() {
    // Given: two cloned register ports share one real device.
    let storage = Arc::new(SharedMemory::new());
    let memory = pdma_model_host::adapter::abi::PdmaHostMemory::new(storage.clone());
    let (device, _worker) = AsyncDevice::new(memory.memory_access(), ResourceConfig::default());
    let (first, second) = SoftwareCpu::new()
        .run_main(async {
            let mut first =
                PdmaRegisterPort::new(AsyncDeviceAccess::new(device.handle(), memory.clone()))
                    .expect("port");
            let mut second = first.clone();
            // When: each clone creates an independent queue.
            let first = first.create_queue(config()).await.expect("first queue");
            let second = second.create_queue(config()).await.expect("second queue");
            (first, second)
        })
        .await;
    // Then: creation reserves distinct queue slots across clones.
    assert_ne!(first.queue_id(), second.queue_id());
}

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
    let storage = Arc::new(SharedMemory::new());
    let memory = pdma_model_host::adapter::abi::PdmaHostMemory::new(storage.clone());
    let (device, _worker) = AsyncDevice::new_observed(
        memory.memory_access(),
        ResourceConfig::default(),
        observer.clone(),
    );
    let handle = device.handle();
    // When: software bootstraps its queue through the register port.
    let status = SoftwareCpu::new()
        .run_main(async {
            let mut port =
                PdmaRegisterPort::new(AsyncDeviceAccess::new(handle.clone(), memory.clone()))
                    .expect("port");
            let queue = port.create_queue(config()).await.expect("queue");
            port.queue_status(EpochQueue::new(queue.epoch(), queue.queue_id()))
                .await
                .expect("status")
        })
        .await;
    // Then: all three mappings persist and the empty queue was activated by direct accesses.
    assert_eq!(storage.revision().raw(), 3);
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
    let storage = Arc::new(SharedMemory::new());
    let memory = pdma_model_host::adapter::abi::PdmaHostMemory::new(storage.clone());
    let (device, _worker) = AsyncDevice::new(memory.memory_access(), ResourceConfig::default());
    let handle = device.handle();
    let access = GatedAccess::new(AsyncDeviceAccess::new(handle.clone(), memory.clone()));
    SoftwareCpu::new()
        .run_main(async {
            let mut port = PdmaRegisterPort::new(access.clone()).expect("port");
            access.arm_write(queue_offset(0, QueueRegisters::ENABLE.offset()).expect("enable"));
            // When: software drops queue creation after its ring allocation and staging writes.
            drop_pending(port.create_queue(config())).await;
            assert!(access.is_blocked());
            // Then: each allocation remains, staged memory is mapped, and no queue was activated.
            assert_eq!(storage.revision().raw(), 3);
            let mut base = [0; 8];
            RegisterAccess::read(
                &access,
                queue_offset(0, QueueRegisters::SQ_BASE.offset()).expect("SQ base offset"),
                &mut base,
            )
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
            access.release();
            let mut enabled = [1; 4];
            RegisterAccess::read(
                &access,
                queue_offset(0, QueueRegisters::ENABLE.offset()).expect("enable"),
                &mut enabled,
            )
            .await
            .expect("inactive queue");
            assert_eq!(enabled, [0; 4]);
        })
        .await;
}

#[expect(
    clippy::redundant_pub_crate,
    reason = "queue configuration is shared only by sibling integration test fixtures"
)]
pub(super) const fn config() -> QueueConfig {
    QueueConfig::new(2, 2, EventRouting::QueueLocal, SoftwareOwnerId::new(0))
}
