#![allow(missing_docs)]

use pdma_model_contract::semantic::{
    CompletionStatus, ResourceConfig,
    desc::{Command, CopyRequest},
    ids::CommandId,
    queue::{EventRouting, QueueConfig, SoftwareOwnerId},
};
use pdma_model_device::{AsyncDevice, AsyncDeviceHandle};

#[path = "runner/dma_io.rs"]
pub mod dma_io;
#[path = "runner/semantic_dma.rs"]
pub mod semantic_dma;
#[path = "runner/shared_io.rs"]
pub mod shared_io;

#[test]
fn dma_bus_queue_selection_routes_decoded_command_through_shared_memory_resolver()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: one live DMA-bus queue and one already-decoded command.
    // When: the command enters the shared execution core before DMA publication exists.
    // Then: the helper proves one execution through queue-selected memory and no semantic result.
    dma_io::assert_dma_bus_decoded_command_uses_shared_core()
}

#[test]
fn semantic_frontend_resource_and_publication_stalls_do_not_consume_or_reexecute()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: two semantic queues sharing constrained execution and publication resources.
    // When: one command resource-stalls while another waits in global pending retirement.
    // Then: the helper proves the first remains queued and the second executes exactly once.
    shared_io::assert_semantic_stall_commit_points()
}

#[tokio::test(flavor = "current_thread")]
async fn semantic_nonzero_start_unknown_object_and_range_overflow_publish_no_success() {
    // Given: real async and sync semantic queue-port frontends with mapped DMA objects.
    // When: explicit-start copy/strided commands and mapped, unknown, and overflowing faults run.
    // Then: the helper asserts exact bytes, typed address correlation, and no false success.
    semantic_dma::exercise_semantic_dma_ranges_and_faults()
        .await
        .expect("semantic DMA behavior");
}

#[tokio::test(flavor = "current_thread")]
async fn async_runner_retires_copy_after_handle_submission() {
    // Given: a queue-port async runner, an async handle, one queue, source/destination buffers, and a source write.
    let (runner, worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::semantic::ResourceConfig::default(),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    let worker = tokio::spawn(worker.run());
    let handle: AsyncDeviceHandle = runner.handle();
    let queue = handle
        .create_queue(QueueConfig::new(
            4,
            4,
            EventRouting::QueueLocal,
            SoftwareOwnerId::new(0),
        ))
        .await
        .expect("queue setup is valid");
    let src = handle.allocate_buffer(4).await.expect("allocation");
    let dst = handle.allocate_buffer(4).await.expect("allocation");
    handle
        .write_buffer(src, 0, &[1, 2, 3, 4])
        .await
        .expect("source write is in bounds");

    // When: the handle submits one semantic copy command, rings the doorbell, and drives retirement.
    let command = Command::new(
        CommandId::new(0),
        pdma_model_contract::semantic::CommandOperation::Copy(CopyRequest::new(src, dst, 4)),
    );
    handle
        .submit_command(queue, command)
        .await
        .expect("copy submission has ring space");
    handle.ring_doorbell(queue).await.expect("doorbell rings");
    let snapshot = runner.wait_for_progress_after(0).await;
    let completion = handle
        .poll_completion(queue)
        .await
        .expect("completion ring exists")
        .expect("completion was published");
    runner.request_shutdown().await;
    let summary = worker.await.expect("runner joins cleanly");

    // Then: completion is success, snapshot/summary steps are 1, and destination bytes equal source.
    assert_eq!(completion.status(), CompletionStatus::Success);
    assert_eq!(snapshot.steps, 1);
    assert_eq!(summary.steps, 1);
    assert_eq!(handle.read_buffer(dst, 0, 4).await, Ok(vec![1, 2, 3, 4]));
}

#[tokio::test(flavor = "current_thread")]
async fn async_runner_pending_completion_retry_after_doorbell_has_no_fake_progress() {
    // Given: a one-slot completion publication path with three direct copy commands on one queue.
    let (runner, worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        ResourceConfig::default().with_completion_publication_slots(1),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    let worker = tokio::spawn(worker.run());
    let handle = runner.handle();
    let queue = handle
        .create_queue(QueueConfig::new(
            4,
            4,
            EventRouting::QueueLocal,
            SoftwareOwnerId::new(0),
        ))
        .await
        .expect("queue setup is valid");
    let src = handle.allocate_buffer(4).await.expect("allocation");
    let first_dst = handle.allocate_buffer(4).await.expect("allocation");
    let second_dst = handle.allocate_buffer(4).await.expect("allocation");
    let third_dst = handle.allocate_buffer(4).await.expect("allocation");
    handle
        .write_buffer(src, 0, &[3, 5, 7, 9])
        .await
        .expect("source write is in bounds");
    for (command_id, dst) in [first_dst, second_dst, third_dst].into_iter().enumerate() {
        let command = Command::new(
            CommandId::new(command_id),
            pdma_model_contract::semantic::CommandOperation::Copy(CopyRequest::new(src, dst, 4)),
        );
        handle
            .submit_command(queue, command)
            .await
            .expect("copy submission has ring space");
    }

    // When: the second completion stalls, software doorbells the queue again, and retirement retries.
    handle.ring_doorbell(queue).await.expect("doorbell rings");
    let first_snapshot = runner.wait_for_progress_after(0).await;
    tokio::task::yield_now().await;
    let stalled_snapshot = runner.snapshot().await;
    handle
        .ring_doorbell(queue)
        .await
        .expect("second doorbell rings while retirement is pending");
    let first_completion = handle
        .poll_completion(queue)
        .await
        .expect("completion ring exists")
        .expect("first completion was published");
    let retry_snapshot = runner.wait_for_progress_after(1).await;
    let second_completion = handle
        .poll_completion(queue)
        .await
        .expect("completion ring exists")
        .expect("second completion was published");
    let third_snapshot = runner.wait_for_progress_after(2).await;
    let third_completion = handle
        .poll_completion(queue)
        .await
        .expect("completion ring exists")
        .expect("third completion was published");
    handle
        .ring_doorbell(queue)
        .await
        .expect("empty queue doorbell is accepted");
    let final_snapshot = runner.wait_for_change_after(third_snapshot).await;
    runner.request_shutdown().await;
    let summary = worker.await.expect("runner joins cleanly");

    // Then: only the three real retirements count as progress and destination buffers are copied once.
    assert_eq!(first_snapshot.steps, 1);
    assert!(stalled_snapshot.stalls >= 1);
    assert_eq!(retry_snapshot.steps, 2);
    assert_eq!(first_completion.command_id(), CommandId::new(0));
    assert_eq!(second_completion.command_id(), CommandId::new(1));
    assert_eq!(third_completion.command_id(), CommandId::new(2));
    assert_eq!(final_snapshot.steps, 3);
    assert_eq!(summary.steps, 3);
    assert_eq!(
        handle.read_buffer(first_dst, 0, 4).await,
        Ok(vec![3, 5, 7, 9])
    );
    assert_eq!(
        handle.read_buffer(second_dst, 0, 4).await,
        Ok(vec![3, 5, 7, 9])
    );
    assert_eq!(
        handle.read_buffer(third_dst, 0, 4).await,
        Ok(vec![3, 5, 7, 9])
    );
}

#[tokio::test(flavor = "current_thread")]
async fn async_runner_uses_selected_queue_completion_routing() {
    // Given: a queue with completion routing and a command containing only identity and operation.
    let (runner, worker) = AsyncDevice::from_core(pdma_model_device::DeviceCore::new(
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new()),
        pdma_model_contract::semantic::ResourceConfig::default(),
        std::sync::Arc::new(hardware_interface_runtime::NoopRegisterObserver),
    ));
    let worker = tokio::spawn(worker.run());
    let handle: AsyncDeviceHandle = runner.handle();
    let queue = handle
        .create_queue(QueueConfig::new(
            4,
            4,
            EventRouting::QueueLocal,
            SoftwareOwnerId::new(0),
        ))
        .await
        .expect("queue setup is valid");
    let src = handle.allocate_buffer(4).await.expect("allocation");
    let dst = handle.allocate_buffer(4).await.expect("allocation");
    handle
        .write_buffer(src, 0, &[8, 6, 4, 2])
        .await
        .expect("source write is in bounds");
    let command = Command::new(
        CommandId::new(7),
        pdma_model_contract::semantic::CommandOperation::Copy(CopyRequest::new(src, dst, 4)),
    );

    // When: the command is submitted and retired through the selected queue.
    handle
        .submit_command(queue, command)
        .await
        .expect("command fits the command ring");
    handle.ring_doorbell(queue).await.expect("doorbell rings");
    let snapshot = runner.wait_for_progress_after(0).await;
    let completion = handle
        .poll_completion(queue)
        .await
        .expect("completion ring exists")
        .expect("completion was published through the selected queue");
    runner.request_shutdown().await;
    let summary = worker.await.expect("runner joins cleanly");

    // Then: the queue publishes the command's completion without command-owned routing state.
    assert_eq!(handle.read_buffer(dst, 0, 4).await, Ok(vec![8, 6, 4, 2]));
    assert_eq!(completion.command_id(), CommandId::new(7));
    assert_eq!(completion.status(), CompletionStatus::Success);
    assert_eq!(snapshot.steps, 1);
    assert_eq!(summary.steps, 1);
    assert_eq!(summary.errors, 0);
}
