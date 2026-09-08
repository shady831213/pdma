use hardware_interface_runtime::{
    MemoryAccess, MemoryAddress, MemoryAlignment, RegisterAccess, SharedMemory, SoftwareCpu,
};
use pdma_model_contract::{
    abi::{AbiCommand, mmio::QueueCommand},
    semantic::{Command, CommandId, CommandOperation, CopyRequest, EpochQueue, ResourceConfig},
};
use pdma_model_device::{AsyncDevice, PdmaMemoryProvisioning};
use pdma_model_host::{
    DriverPort,
    adapter::abi::{AsyncDeviceAccess, PdmaIrqCoordinator, PdmaRegisterPort},
};
use std::sync::Arc;

use super::super::pending_commands::support::{GatedAccess, drop_pending};
use super::config;

#[tokio::test(flavor = "current_thread")]
async fn canceled_publication_preserves_descriptor_bytes_without_accepting_work() {
    // Given: software prepares a real copy while producer publication is suspended.
    let (device, worker) = AsyncDevice::new(
        PdmaMemoryProvisioning::shared(Arc::new(SharedMemory::new())),
        ResourceConfig::default(),
    );
    let worker = tokio::spawn(worker.run());
    let handle = device.handle();
    let access = GatedAccess::new(AsyncDeviceAccess::new(
        handle.clone(),
        PdmaIrqCoordinator::default(),
    ));
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

#[tokio::test(flavor = "current_thread")]
async fn full_queue_rejection_preserves_previously_published_descriptor() {
    // Given: two accepted descriptors occupy every slot before doorbell capture.
    let (device, worker) = AsyncDevice::new(
        PdmaMemoryProvisioning::shared(Arc::new(SharedMemory::new())),
        ResourceConfig::default(),
    );
    let worker = tokio::spawn(worker.run());
    let access = AsyncDeviceAccess::new(device.handle(), PdmaIrqCoordinator::default());
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
