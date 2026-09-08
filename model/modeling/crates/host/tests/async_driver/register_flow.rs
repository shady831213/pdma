use hardware_interface_runtime::{
    AccessOrigin, RegisterObservation, RegisterObserver, SharedMemory, SoftwareCpu,
};
use pdma_model_contract::semantic::{EventRouting, QueueConfig, ResourceConfig, SoftwareOwnerId};
use pdma_model_device::{AsyncDevice, DeviceCore};
use pdma_model_host::{
    DriverPort,
    adapter::abi::{AsyncDeviceAccess, PdmaRegisterPort},
};
use std::sync::{Arc, Mutex};

use super::pending_commands::support::{GatedAccess, drop_pending};
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment, RegisterAccess};
use pdma_model_contract::{
    abi::{AbiCommand, mmio::QueueCommand},
    semantic::{Command, CommandId, CommandOperation, CopyRequest, EpochQueue, QueueControl},
};

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
    let (device, _worker) = AsyncDevice::from_core_observed(
        DeviceCore::new(Arc::new(SharedMemory::new()), ResourceConfig::default()),
        observer.clone(),
    );
    let handle = device.handle();
    // When: software bootstraps its queue through the register port.
    let status = SoftwareCpu::new()
        .run_main(async {
            let mut port =
                PdmaRegisterPort::new(AsyncDeviceAccess::new(handle.clone())).expect("port");
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
    let (device, _worker) = AsyncDevice::from_core(DeviceCore::new(
        Arc::new(SharedMemory::new()),
        ResourceConfig::default(),
    ));
    let handle = device.handle();
    let access = GatedAccess::new(AsyncDeviceAccess::new(handle.clone()));
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

#[tokio::test(flavor = "current_thread")]
async fn canceled_publication_preserves_descriptor_bytes_without_accepting_work() {
    // Given: software prepares a real copy while producer publication is suspended.
    let (device, worker) = AsyncDevice::from_core(DeviceCore::new(
        Arc::new(SharedMemory::new()),
        ResourceConfig::default(),
    ));
    let worker = tokio::spawn(worker.run());
    let handle = device.handle();
    let access = GatedAccess::new(AsyncDeviceAccess::new(handle.clone()));
    SoftwareCpu::new()
        .run_main(async {
            let mut port = PdmaRegisterPort::new(access.clone()).expect("port");
            let handle = port.create_queue(config()).await.expect("queue");
            let queue = EpochQueue::new(handle.epoch(), handle.queue_id());
            let src = port.allocate_buffer(4).await.expect("source");
            let dst = port.allocate_buffer(4).await.expect("destination");
            port.write_buffer(src, 0, &[1, 2, 3, 4])
                .await
                .expect("source bytes");
            let command = |id| {
                Command::new(
                    CommandId::new(id),
                    CommandOperation::Copy(CopyRequest::new(src, dst, 4)),
                )
            };
            access.arm_before(QueueCommand::PublishSqProducer.code());
            // When: the future is dropped after writing its descriptor but before publication.
            drop_pending(port.submit_command(handle, command(91))).await;
            assert!(access.is_blocked());
            // Then: bytes persist, producer stays zero, and a fresh submission completes once.
            let mut base = [0; 8];
            RegisterAccess::read(&access, 0x138, &mut base)
                .await
                .expect("SQ base");
            let mut bytes = [0; 64];
            MemoryAccess::read(
                &access,
                MemoryAddress::new(u64::from_le_bytes(base)),
                MemoryAlignment::ONE,
                &mut bytes,
            )
            .expect("descriptor bytes");
            let descriptor = AbiCommand::decode(&bytes).expect("descriptor");
            assert_eq!(descriptor.header.command_id, 91);
            access.release();
            assert_eq!(
                port.queue_status(queue)
                    .await
                    .expect("status")
                    .submission()
                    .producer()
                    .raw(),
                0
            );
            port.submit_command(handle, command(92))
                .await
                .expect("fresh publication");
            port.ring_doorbell(handle).await.expect("doorbell");
            device.wait_for_progress_after(0).await;
            let record = port
                .poll_completion(handle)
                .await
                .expect("completion")
                .expect("record");
            assert_eq!(record.command_id(), CommandId::new(92));
            assert!(
                port.poll_completion(handle)
                    .await
                    .expect("drained")
                    .is_none()
            );
            assert_eq!(
                port.read_buffer(dst, 0, 4).await.expect("copied bytes"),
                vec![1, 2, 3, 4]
            );
        })
        .await;
    device.request_shutdown().await;
    worker.await.expect("worker joined");
}

const fn config() -> QueueConfig {
    QueueConfig::new(2, 2, EventRouting::QueueLocal, SoftwareOwnerId::new(0))
}

#[tokio::test(flavor = "current_thread")]
async fn full_queue_rejection_preserves_previously_published_descriptor() {
    // Given: two accepted descriptors occupy every slot before doorbell capture.
    let (device, worker) = AsyncDevice::from_core(DeviceCore::new(
        Arc::new(SharedMemory::new()),
        ResourceConfig::default(),
    ));
    let worker = tokio::spawn(worker.run());
    let access = AsyncDeviceAccess::new(device.handle());
    SoftwareCpu::new()
        .run_main(async {
            let mut port = PdmaRegisterPort::new(access.clone()).expect("port");
            let queue = port.create_queue(config()).await.expect("queue");
            let src = port.allocate_buffer(1).await.expect("source");
            let dst = port.allocate_buffer(1).await.expect("destination");
            let command = |id| {
                Command::new(
                    CommandId::new(id),
                    CommandOperation::Copy(CopyRequest::new(src, dst, 1)),
                )
            };
            port.submit_command(queue, command(31))
                .await
                .expect("first");
            port.submit_command(queue, command(32))
                .await
                .expect("second");
            // When: a third submission is rejected by the full ring.
            assert!(port.submit_command(queue, command(33)).await.is_err());
            // Then: the first published slot retains its original descriptor.
            let mut base = [0; 8];
            RegisterAccess::read(&access, 0x138, &mut base)
                .await
                .expect("SQ base");
            let mut bytes = [0; 64];
            MemoryAccess::read(
                &access,
                MemoryAddress::new(u64::from_le_bytes(base)),
                MemoryAlignment::ONE,
                &mut bytes,
            )
            .expect("descriptor");
            assert_eq!(
                AbiCommand::decode(&bytes)
                    .expect("valid descriptor")
                    .header
                    .command_id,
                31
            );
            port.ring_doorbell(queue).await.expect("doorbell");
            device.wait_for_progress_after(1).await;
            for id in [31, 32] {
                let record = port
                    .poll_completion(queue)
                    .await
                    .expect("completion")
                    .expect("record");
                assert_eq!(record.command_id(), CommandId::new(id));
            }
            assert!(
                port.poll_completion(queue)
                    .await
                    .expect("drained")
                    .is_none()
            );
        })
        .await;
    device.request_shutdown().await;
    worker.await.expect("worker joined");
}
