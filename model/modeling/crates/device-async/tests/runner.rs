#![allow(missing_docs)]

use pdma_model_contract::{
    CompletionStatus,
    desc::{CopyCommand, CopyRequest, ExternalCommand},
    ids::CommandId,
    queue::{EventRouting, QueueConfig, SoftwareOwnerId},
};
use pdma_model_device_async::{AsyncDeviceHandle, AsyncDeviceRunner};

#[tokio::test(flavor = "current_thread")]
async fn async_runner_retires_copy_after_handle_submission() {
    // Given: a native async runner, an async handle, one queue, source/destination buffers, and a source write.
    let runner: AsyncDeviceRunner = AsyncDeviceRunner::spawn_native();
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
        .submit_command(queue.queue_id(), command)
        .await
        .expect("copy submission has ring space");
    handle
        .ring_doorbell(queue.queue_id())
        .await
        .expect("doorbell rings");
    let snapshot = runner.wait_for_progress_after(0).await;
    let completion = handle
        .poll_completion(queue.completion_ring_id())
        .await
        .expect("completion ring exists")
        .expect("completion was published");
    runner.request_shutdown().await;
    let summary = runner.join().await.expect("runner joins cleanly");

    // Then: completion is success, snapshot/summary steps are 1, and destination bytes equal source.
    assert_eq!(completion.status(), CompletionStatus::Success);
    assert_eq!(snapshot.steps, 1);
    assert_eq!(summary.steps, 1);
    assert_eq!(handle.read_buffer(dst, 0, 4).await, Ok(vec![1, 2, 3, 4]));
}
