#![allow(missing_docs)]

use pdma_model_contract::{
    CompletionStatus, ResourceConfig,
    desc::{CopyCommand, CopyRequest, ExternalCommand},
    ids::{CommandId, CompletionRingId},
    queue::{EventRouting, QueueConfig, SoftwareOwnerId},
};
use pdma_model_device::{AsyncDevice, AsyncDeviceHandle};

#[tokio::test(flavor = "current_thread")]
async fn async_runner_retires_copy_after_handle_submission() {
    // Given: a queue-port async runner, an async handle, one queue, source/destination buffers, and a source write.
    let (runner, worker) = AsyncDevice::queue_port();
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
    let src = handle.allocate_buffer(4).await;
    let dst = handle.allocate_buffer(4).await;
    handle
        .write_buffer(src, 0, &[1, 2, 3, 4])
        .await
        .expect("source write is in bounds");

    // When: the handle submits one ExternalCommand::Copy or equivalent command path directly to the async device handle, rings the doorbell if needed, waits for one progress step, polls completion, requests shutdown, and joins.
    let command = ExternalCommand::Copy(CopyCommand::new(
        CommandId::new(0),
        queue.queue_id(),
        queue.completion_ring_id(),
        CopyRequest::new(src, dst, 4),
    ));
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
    let (runner, worker) = AsyncDevice::queue_port_with_resource_config(
        ResourceConfig::default().with_completion_publication_slots(1),
    );
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
    let src = handle.allocate_buffer(4).await;
    let first_dst = handle.allocate_buffer(4).await;
    let second_dst = handle.allocate_buffer(4).await;
    let third_dst = handle.allocate_buffer(4).await;
    handle
        .write_buffer(src, 0, &[3, 5, 7, 9])
        .await
        .expect("source write is in bounds");
    for (command_id, dst) in [first_dst, second_dst, third_dst].into_iter().enumerate() {
        let command = ExternalCommand::Copy(CopyCommand::new(
            CommandId::new(command_id),
            queue.queue_id(),
            queue.completion_ring_id(),
            CopyRequest::new(src, dst, 4),
        ));
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
async fn async_runner_malformed_direct_command_unknown_completion_ring_has_no_side_effects() {
    // Given: a direct copy command whose completion ring is not present in the async device.
    let (runner, worker) = AsyncDevice::queue_port();
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
    let src = handle.allocate_buffer(4).await;
    let dst = handle.allocate_buffer(4).await;
    handle
        .write_buffer(src, 0, &[8, 6, 4, 2])
        .await
        .expect("source write is in bounds");
    let malformed = ExternalCommand::Copy(CopyCommand::new(
        CommandId::new(7),
        queue.queue_id(),
        CompletionRingId::new(99),
        CopyRequest::new(src, dst, 4),
    ));

    // When: the malformed command is submitted, doorbelled, and doorbelled again after the error.
    handle
        .submit_command(queue, malformed)
        .await
        .expect("malformed command still fits the command ring");
    handle.ring_doorbell(queue).await.expect("doorbell rings");
    let error_snapshot = runner.wait_for_error_after(0).await;
    handle
        .ring_doorbell(queue)
        .await
        .expect("second doorbell rings after malformed command handling");
    let final_snapshot = runner.wait_for_change_after(error_snapshot).await;
    runner.request_shutdown().await;
    let summary = worker.await.expect("runner joins cleanly");

    // Then: destination memory is untouched, no valid completion is published, and no retry zombie remains.
    assert_eq!(handle.read_buffer(dst, 0, 4).await, Ok(vec![0, 0, 0, 0]));
    assert_eq!(handle.poll_completion(queue).await, Ok(None));
    assert_eq!(error_snapshot.errors, 1);
    assert_eq!(final_snapshot.errors, 1);
    assert_eq!(summary.errors, 1);
    assert_eq!(summary.steps, 0);
}
