use hardware_interface_runtime::{SharedMemory, SoftwareCpu};
use pdma_model_contract::semantic::{CommandId, ResourceConfig};
use pdma_model_device::AsyncDevice;
use pdma_model_host::{
    Command, CommandOperation, CopyRequest, DriverPort,
    adapter::abi::{AsyncDeviceAccess, PdmaRegisterPort},
};
use std::sync::Arc;

#[tokio::test(flavor = "current_thread")]
async fn cloned_ports_keep_two_queues_and_their_completions_independent() {
    // Given: cloned register ports configure two real hardware queues.
    let storage = Arc::new(SharedMemory::new());
    let memory = pdma_model_host::adapter::abi::PdmaHostMemory::new(storage.clone());
    let (device, worker) = AsyncDevice::new(memory.memory_access(), ResourceConfig::default());
    let worker = tokio::spawn(worker.run());
    SoftwareCpu::new()
        .run_main(async {
            let mut first =
                PdmaRegisterPort::new(AsyncDeviceAccess::new(device.handle(), memory.clone()))
                    .expect("port");
            let mut second = first.clone();
            let first_queue = first
                .create_queue(super::config())
                .await
                .expect("first queue");
            let second_queue = second
                .create_queue(super::config())
                .await
                .expect("second queue");
            let source = first.allocate_buffer(2).await.expect("source");
            let first_dst = first.allocate_buffer(2).await.expect("first destination");
            let second_dst = second.allocate_buffer(2).await.expect("second destination");
            first
                .write_buffer(source, 0, &[4, 7])
                .await
                .expect("source bytes");

            // When: each clone publishes and rings only its own queue.
            first
                .submit_command(
                    first_queue,
                    Command::new(
                        CommandId::new(11),
                        CommandOperation::Copy(CopyRequest::new(source, first_dst, 1)),
                    ),
                )
                .await
                .expect("first submit");
            second
                .submit_command(
                    second_queue,
                    Command::new(
                        CommandId::new(22),
                        CommandOperation::Copy(CopyRequest::new(source, second_dst, 2)),
                    ),
                )
                .await
                .expect("second submit");
            second
                .ring_doorbell(second_queue)
                .await
                .expect("second doorbell");
            first
                .ring_doorbell(first_queue)
                .await
                .expect("first doorbell");
            device.wait_for_progress_after(1).await;

            // Then: records and destination bytes remain correlated with their own queues.
            assert_ne!(first_queue.queue_id(), second_queue.queue_id());
            assert_eq!(
                first
                    .poll_completion(first_queue)
                    .await
                    .expect("first completion")
                    .expect("first record")
                    .command_id(),
                CommandId::new(11)
            );
            assert_eq!(
                second
                    .poll_completion(second_queue)
                    .await
                    .expect("second completion")
                    .expect("second record")
                    .command_id(),
                CommandId::new(22)
            );
            assert_eq!(
                second
                    .read_buffer(first_dst, 0, 2)
                    .await
                    .expect("first bytes"),
                [4, 0]
            );
            assert_eq!(
                first
                    .read_buffer(second_dst, 0, 2)
                    .await
                    .expect("second bytes"),
                [4, 7]
            );
            let first_drained = first
                .poll_completion(first_queue)
                .await
                .expect("first drained");
            let second_drained = second
                .poll_completion(second_queue)
                .await
                .expect("second drained");
            assert_eq!((first_drained, second_drained), (None, None));
        })
        .await;
    device.request_shutdown().await;
    worker.await.expect("worker");
}
