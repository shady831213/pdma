use hardware_interface_runtime::{RegisterAccess, SharedMemory, SoftwareCpu};
use pdma_model_contract::{
    abi::registers::GlobalRegisters,
    semantic::{CommandId, DeviceEpoch, ResourceConfig},
};
use pdma_model_device::AsyncDevice;
use pdma_model_host::{
    Command, CommandOperation, CopyRequest, DriverPort, PortError,
    adapter::abi::{AsyncDeviceAccess, PdmaRegisterPort},
};
use std::sync::Arc;

#[tokio::test(flavor = "current_thread")]
async fn stale_queue_and_result_capabilities_commit_nothing() {
    // Given: a queue capability retained across whole-device reset.
    let storage = Arc::new(SharedMemory::new());
    let memory = pdma_model_host::adapter::abi::PdmaHostMemory::new(storage.clone());
    let (device, _worker) = AsyncDevice::new(memory.memory_access(), ResourceConfig::default());
    let handle = device.handle();
    SoftwareCpu::new()
        .run_main(async {
            let access = AsyncDeviceAccess::new(handle.clone(), memory.clone());
            let mut old = PdmaRegisterPort::new(access.clone()).expect("port");
            let queue = old
                .create_queue(super::register_flow::config())
                .await
                .expect("queue");
            let src = old.allocate_buffer(1).await.expect("source");
            let dst = old.allocate_buffer(1).await.expect("destination");
            old.write_buffer(src, 0, &[7]).await.expect("source bytes");
            RegisterAccess::write(
                &access,
                u32::try_from(GlobalRegisters::RESET.offset()).expect("reset offset"),
                &1_u32.to_le_bytes(),
            )
            .await
            .expect("reset");
            let epoch = DeviceEpoch::initial().next().expect("next epoch");
            let command = Command::new(
                CommandId::new(0),
                CommandOperation::Copy(CopyRequest::new(src, dst, 1)),
            );
            let before = storage.revision().raw();

            // When: old capabilities attempt queue, IRQ, and result operations.
            let results = [
                old.submit_command(queue, command).await,
                old.ring_doorbell(queue).await,
                old.poll_completion(queue).await.map(|_| ()),
                old.poll_queue_event(queue).await.map(|_| ()),
                old.enable_irq_source(queue.completion_irq_source()).await,
            ];

            // Then: typed epoch errors precede mutation and explicit bootstrap creates a fresh queue.
            for result in results {
                assert_eq!(
                    result,
                    Err(PortError::StaleEpoch {
                        expected: epoch,
                        actual: DeviceEpoch::initial()
                    })
                );
            }
            assert_eq!(storage.revision().raw(), before);
            assert_eq!(
                old.read_buffer(src, 0, 1).await.expect("persistent bytes"),
                [7]
            );
            let mut fresh = PdmaRegisterPort::new_at(access, epoch).expect("new epoch port");
            let recreated = fresh
                .create_queue(super::register_flow::config())
                .await
                .expect("new queue");
            assert_eq!(recreated.epoch(), epoch);
        })
        .await;
}
