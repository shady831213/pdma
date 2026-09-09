#![allow(missing_docs)]

use hardware_interface_runtime::SoftwareCpu;
use pdma_model_contract::semantic::{CompletionStatus, EventErrorKind};
use pdma_model_device::AsyncDevice;
use pdma_model_host::{
    AsyncDriver,
    adapter::abi::{AsyncDeviceAccess, PdmaRegisterPort},
};
use pdma_model_host::{BroadcastCopyRequest, BroadcastDestinationRange};

#[tokio::test(flavor = "current_thread")]
async fn async_runner_broadcasts_one_source_range_to_multiple_destinations_and_publishes_one_completion()
 {
    // Given: async runner handle, AsyncDriver, one source range, and two destination ranges.
    let host_memory = pdma_model_host::adapter::abi::PdmaHostMemory::new(std::sync::Arc::new(
        hardware_interface_runtime::SharedMemory::new(),
    ));
    let (runner, worker) = AsyncDevice::new(
        host_memory.memory_access(),
        pdma_model_contract::semantic::ResourceConfig::default(),
    );
    let worker = tokio::spawn(worker.run());
    let (snapshot, completion, command_id, no_completion, left_bytes, right_bytes) =
        SoftwareCpu::new()
            .run_main(async {
                let mut handle = PdmaRegisterPort::new(AsyncDeviceAccess::new(
                    runner.handle(),
                    host_memory.clone(),
                ))
                .expect("active software CPU");
                let mut driver = AsyncDriver::setup_on(&mut handle, 2, 4)
                    .await
                    .expect("queue setup is valid");
                let src = driver
                    .allocate_buffer_on(&mut handle, 8)
                    .await
                    .expect("allocation");
                let dst_a = driver
                    .allocate_buffer_on(&mut handle, 8)
                    .await
                    .expect("allocation");
                let dst_b = driver
                    .allocate_buffer_on(&mut handle, 8)
                    .await
                    .expect("allocation");
                driver
                    .write_buffer_on(&mut handle, src, &[0, 1, 2, 3, 4, 5, 6, 7])
                    .await
                    .expect("source write is in bounds");

                // When: the software main submits the broadcast, waits for progress, and polls completion.
                let command_id = driver
                    .submit_broadcast_copy_on(
                        &mut handle,
                        BroadcastCopyRequest::new(
                            src,
                            2,
                            4,
                            vec![
                                BroadcastDestinationRange::new(dst_a, 0),
                                BroadcastDestinationRange::new(dst_b, 3),
                            ],
                        ),
                    )
                    .await
                    .expect("broadcast submission has ring space");
                let snapshot = runner.wait_for_progress_after(0).await;
                let completion = driver
                    .poll_completion_on(&mut handle)
                    .await
                    .expect("completion ring exists")
                    .expect("broadcast published one completion");
                let no_completion = driver.poll_completion_on(&mut handle).await;
                let left_bytes = driver.read_buffer_on(&handle, dst_a, 8).await;
                let right_bytes = driver.read_buffer_on(&handle, dst_b, 8).await;
                (
                    snapshot,
                    completion,
                    command_id,
                    no_completion,
                    left_bytes,
                    right_bytes,
                )
            })
            .await;
    runner.request_shutdown().await;
    let summary = worker.await.expect("runner joins cleanly");

    // Then: both destinations receive the source bytes and only one completion appears.
    assert_eq!(snapshot.steps, 1);
    assert_eq!(completion.command_id(), command_id);
    assert_eq!(completion.status(), CompletionStatus::Success);
    assert_eq!(no_completion, Ok(None));
    assert_eq!(left_bytes, Ok(vec![2, 3, 4, 5, 0, 0, 0, 0]));
    assert_eq!(right_bytes, Ok(vec![0, 0, 0, 2, 3, 4, 5, 0]));
    assert_eq!(summary.steps, 1);
}

#[tokio::test(flavor = "current_thread")]
async fn async_runner_publishes_event_and_no_completion_when_broadcast_destination_write_fails() {
    // Given: async runner handle, AsyncDriver, one valid destination, and one too-small destination.
    let host_memory = pdma_model_host::adapter::abi::PdmaHostMemory::new(std::sync::Arc::new(
        hardware_interface_runtime::SharedMemory::new(),
    ));
    let (runner, worker) = AsyncDevice::new(
        host_memory.memory_access(),
        pdma_model_contract::semantic::ResourceConfig::default(),
    );
    let worker = tokio::spawn(worker.run());
    let (snapshot, event, command_id, bad_dst, no_completion, destinations) = SoftwareCpu::new()
        .run_main(async {
            let mut handle =
                PdmaRegisterPort::new(AsyncDeviceAccess::new(runner.handle(), host_memory.clone()))
                    .expect("active software CPU");
            let mut driver = AsyncDriver::setup_on(&mut handle, 2, 4)
                .await
                .expect("queue setup is valid");
            let src = driver
                .allocate_buffer_on(&mut handle, 4)
                .await
                .expect("allocation");
            let valid_dst = driver
                .allocate_buffer_on(&mut handle, 4)
                .await
                .expect("allocation");
            let bad_dst = driver
                .allocate_buffer_on(&mut handle, 2)
                .await
                .expect("allocation");
            driver
                .write_buffer_on(&mut handle, src, &[9, 8, 7, 6])
                .await
                .expect("source write is in bounds");
            driver
                .write_buffer_on(&mut handle, valid_dst, &[0x55; 4])
                .await
                .expect("first sentinel");
            driver
                .write_buffer_on(&mut handle, bad_dst, &[0x66; 2])
                .await
                .expect("second sentinel");

            // When: a broadcast command writes past the second destination.
            let command_id = driver
                .submit_broadcast_copy_on(
                    &mut handle,
                    BroadcastCopyRequest::new(
                        src,
                        0,
                        4,
                        vec![
                            BroadcastDestinationRange::new(valid_dst, 0),
                            BroadcastDestinationRange::new(bad_dst, 0),
                        ],
                    ),
                )
                .await
                .expect("broadcast submission has ring space");
            let snapshot = runner.wait_for_error_after(0).await;
            let event = driver
                .poll_event_on(&mut handle)
                .await
                .expect("event ring exists")
                .expect("broadcast fault event was published");
            let no_completion = driver.poll_completion_on(&mut handle).await;
            let destinations = (
                driver.read_buffer_on(&handle, valid_dst, 4).await,
                driver.read_buffer_on(&handle, bad_dst, 2).await,
            );
            (
                snapshot,
                event,
                command_id,
                bad_dst,
                no_completion,
                destinations,
            )
        })
        .await;
    runner.request_shutdown().await;
    let summary = worker.await.expect("runner joins cleanly");

    // Then: the original error is returned, one event is published, and no success completes.
    assert_eq!(snapshot.errors, 1);
    assert_eq!(event.command_id(), command_id);
    assert_eq!(event.buffer(), bad_dst);
    assert_eq!(destinations, (Ok(vec![0x55; 4]), Ok(vec![0x66; 2])));
    assert_eq!(
        event.error_kind(),
        EventErrorKind::OutOfBounds { len: 2, end: 4 }
    );
    assert_eq!(no_completion, Ok(None));
    assert_eq!(summary.errors, 1);
}

#[tokio::test(flavor = "current_thread")]
async fn async_runner_rereads_broadcast_source_for_each_destination_when_ranges_overlap() {
    // Given: a broadcast whose first destination overlaps and mutates the source range.
    let host_memory = pdma_model_host::adapter::abi::PdmaHostMemory::new(std::sync::Arc::new(
        hardware_interface_runtime::SharedMemory::new(),
    ));
    let (runner, worker) = AsyncDevice::new(
        host_memory.memory_access(),
        pdma_model_contract::semantic::ResourceConfig::default(),
    );
    let worker = tokio::spawn(worker.run());
    let (snapshot, completion, command_id, src_bytes, dst_bytes) = SoftwareCpu::new()
        .run_main(async {
            let mut handle =
                PdmaRegisterPort::new(AsyncDeviceAccess::new(runner.handle(), host_memory.clone()))
                    .expect("active software CPU");
            let mut driver = AsyncDriver::setup_on(&mut handle, 2, 4)
                .await
                .expect("queue setup is valid");
            let src = driver
                .allocate_buffer_on(&mut handle, 4)
                .await
                .expect("allocation");
            let dst = driver
                .allocate_buffer_on(&mut handle, 4)
                .await
                .expect("allocation");
            driver
                .write_buffer_on(&mut handle, src, &[1, 2, 3, 4])
                .await
                .expect("source write is in bounds");

            // When: the queue-port runner executes the overlapping broadcast copy.
            let command_id = driver
                .submit_broadcast_copy_on(
                    &mut handle,
                    BroadcastCopyRequest::new(
                        src,
                        0,
                        2,
                        vec![
                            BroadcastDestinationRange::new(src, 1),
                            BroadcastDestinationRange::new(dst, 0),
                        ],
                    ),
                )
                .await
                .expect("broadcast submission has ring space");
            let snapshot = runner.wait_for_progress_after(0).await;
            let completion = driver
                .poll_completion_on(&mut handle)
                .await
                .expect("completion ring exists")
                .expect("broadcast published one completion");
            let src_bytes = driver.read_buffer_on(&handle, src, 4).await;
            let dst_bytes = driver.read_buffer_on(&handle, dst, 4).await;
            (snapshot, completion, command_id, src_bytes, dst_bytes)
        })
        .await;
    runner.request_shutdown().await;
    let summary = worker.await.expect("runner joins cleanly");

    // Then: the second destination observes the source after the first destination write.
    assert_eq!(snapshot.steps, 1);
    assert_eq!(completion.command_id(), command_id);
    assert_eq!(completion.status(), CompletionStatus::Success);
    assert_eq!(src_bytes, Ok(vec![1, 1, 2, 4]));
    assert_eq!(dst_bytes, Ok(vec![1, 1, 0, 0]));
    assert_eq!(summary.steps, 1);
}
