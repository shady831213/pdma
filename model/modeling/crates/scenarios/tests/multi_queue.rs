#![allow(missing_docs)]

use pdma_model_contract::{
    CompletionStatus, TraceEvent, TraceEventKind,
    desc::CopyRequest,
    ids::{CommandId, QueueId},
};
use pdma_model_device::AsyncDevice;
use pdma_model_host::AsyncDriver;

#[tokio::test(flavor = "current_thread")]
async fn async_runner_default_policy_retires_runnable_queues_in_submission_order() {
    // Given: two async queues with one runnable copy each, submitted in queue order.
    let runner = AsyncDevice::spawn_native();
    let mut handle = runner.handle();
    let mut driver_a = AsyncDriver::setup_on(&mut handle, 2, 2)
        .await
        .expect("queue A setup is valid");
    let mut driver_b = AsyncDriver::setup_on(&mut handle, 2, 2)
        .await
        .expect("queue B setup is valid");
    handle.enable_tracing().await;
    handle.clear_trace().await;
    let src_a = driver_a.allocate_buffer_on(&mut handle, 1).await;
    let dst_a = driver_a.allocate_buffer_on(&mut handle, 1).await;
    let src_b = driver_b.allocate_buffer_on(&mut handle, 1).await;
    let dst_b = driver_b.allocate_buffer_on(&mut handle, 1).await;
    driver_a
        .write_buffer_on(&mut handle, src_a, &[10])
        .await
        .expect("source A write is in bounds");
    driver_b
        .write_buffer_on(&mut handle, src_b, &[20])
        .await
        .expect("source B write is in bounds");
    let command_a = driver_a
        .submit_copy_on(&mut handle, CopyRequest::new(src_a, dst_a, 1))
        .await
        .expect("queue A submission has ring space");
    let command_b = driver_b
        .submit_copy_on(&mut handle, CopyRequest::new(src_b, dst_b, 1))
        .await
        .expect("queue B submission has ring space");

    // When: the async runner records both progress steps.
    wait_for_steps(&runner, 2).await;
    let trace_events = handle.trace_events().await;
    let completion_a = driver_a
        .poll_completion_on(&mut handle)
        .await
        .expect("queue A completion ring exists")
        .expect("queue A completion is published");
    let completion_b = driver_b
        .poll_completion_on(&mut handle)
        .await
        .expect("queue B completion ring exists")
        .expect("queue B completion is published");
    runner.request_shutdown().await;
    let summary = runner.join().await.expect("runner joins cleanly");

    // Then: the default policy schedules the oldest runnable queue first.
    let scheduled_a = scheduled_position(&trace_events, driver_a.queue_id(), command_a)
        .expect("queue A scheduled event is traced");
    let scheduled_b = scheduled_position(&trace_events, driver_b.queue_id(), command_b)
        .expect("queue B scheduled event is traced");
    assert!(scheduled_a < scheduled_b);
    assert_eq!(summary.steps, 2);
    assert_eq!(completion_a.command_id(), command_a);
    assert_eq!(completion_a.status(), CompletionStatus::Success);
    assert_eq!(
        driver_a.read_buffer_on(&handle, dst_a, 1).await,
        Ok(vec![10])
    );
    assert_eq!(completion_b.command_id(), command_b);
    assert_eq!(completion_b.status(), CompletionStatus::Success);
    assert_eq!(
        driver_b.read_buffer_on(&handle, dst_b, 1).await,
        Ok(vec![20])
    );
}

#[tokio::test(flavor = "current_thread")]
async fn async_runner_round_robin_does_not_repeat_queue_while_peer_is_runnable() {
    // Given: one peer queue and one repeated-work queue are both runnable.
    let runner = AsyncDevice::spawn_native();
    let mut handle = runner.handle();
    let mut peer = AsyncDriver::setup_on(&mut handle, 1, 2)
        .await
        .expect("peer queue setup is valid");
    let mut repeated = AsyncDriver::setup_on(&mut handle, 2, 3)
        .await
        .expect("repeated queue setup is valid");
    handle.enable_tracing().await;
    handle.clear_trace().await;
    let peer_src = peer.allocate_buffer_on(&mut handle, 1).await;
    let peer_dst = peer.allocate_buffer_on(&mut handle, 1).await;
    let repeated_src = repeated.allocate_buffer_on(&mut handle, 2).await;
    let repeated_dst = repeated.allocate_buffer_on(&mut handle, 2).await;
    peer.write_buffer_on(&mut handle, peer_src, &[30])
        .await
        .expect("peer source write is in bounds");
    repeated
        .write_buffer_on(&mut handle, repeated_src, &[40, 50])
        .await
        .expect("repeated source write is in bounds");
    let peer_command = peer
        .submit_copy_on(&mut handle, CopyRequest::new(peer_src, peer_dst, 1))
        .await
        .expect("peer submission has ring space");
    let repeated_first = repeated
        .submit_copy_on(&mut handle, CopyRequest::new(repeated_src, repeated_dst, 1))
        .await
        .expect("first repeated submission has ring space");
    let repeated_second = repeated
        .submit_copy_on(&mut handle, CopyRequest::new(repeated_src, repeated_dst, 2))
        .await
        .expect("second repeated submission has ring space");

    // When: the async runner records all three progress steps.
    wait_for_steps(&runner, 3).await;
    let trace_events = handle.trace_events().await;
    let peer_completion = peer
        .poll_completion_on(&mut handle)
        .await
        .expect("peer completion ring exists")
        .expect("peer queue is not starved");
    let repeated_completion = repeated
        .poll_completion_on(&mut handle)
        .await
        .expect("repeated completion ring exists")
        .expect("repeated queue first command completes");

    let second_completion = repeated
        .poll_completion_on(&mut handle)
        .await
        .expect("repeated completion ring exists")
        .expect("repeated queue second command completes third");
    runner.request_shutdown().await;
    let summary = runner.join().await.expect("runner joins cleanly");

    // Then: each runnable queue is scheduled once before the repeated queue runs again.
    let peer_scheduled = scheduled_position(&trace_events, peer.queue_id(), peer_command)
        .expect("peer scheduled event is traced");
    let repeated_first_scheduled =
        scheduled_position(&trace_events, repeated.queue_id(), repeated_first)
            .expect("first repeated scheduled event is traced");
    let repeated_second_scheduled =
        scheduled_position(&trace_events, repeated.queue_id(), repeated_second)
            .expect("second repeated scheduled event is traced");
    assert!(peer_scheduled < repeated_first_scheduled);
    assert!(repeated_first_scheduled < repeated_second_scheduled);
    assert_eq!(summary.steps, 3);
    assert_eq!(peer_completion.command_id(), peer_command);
    assert_eq!(repeated_completion.command_id(), repeated_first);
    assert_eq!(
        peer.read_buffer_on(&handle, peer_dst, 1).await,
        Ok(vec![30])
    );
    assert_eq!(
        repeated.read_buffer_on(&handle, repeated_dst, 2).await,
        Ok(vec![40, 50])
    );

    assert_eq!(second_completion.command_id(), repeated_second);
}

async fn wait_for_steps(runner: &AsyncDevice, steps: usize) {
    loop {
        let snapshot = runner.snapshot().await;
        if snapshot.steps >= steps {
            return;
        }
        let _snapshot = runner.wait_for_progress_after(snapshot.steps).await;
    }
}

fn scheduled_position(events: &[TraceEvent], queue: QueueId, command: CommandId) -> Option<usize> {
    events.iter().position(|event| {
        event.kind() == TraceEventKind::Scheduled
            && event.queue_id() == Some(queue)
            && event.command_id() == Some(command)
    })
}
