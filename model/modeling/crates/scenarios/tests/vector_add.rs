#![allow(missing_docs)]

use hardware_interface_runtime::SoftwareCpu;
use pdma_model_contract::semantic::{
    CompletionStatus, EventErrorKind,
    desc::{VectorAddRequest, VectorBufferRange},
};
use pdma_model_device::AsyncDevice;
use pdma_model_host::{
    AsyncDriver, adapter::abi::AsyncDeviceAccess, adapter::abi::PdmaRegisterPort,
};

#[tokio::test(flavor = "current_thread")]
async fn async_runner_adds_bytes_with_wraparound_and_publishes_one_completion() {
    // Given: async runner handle, AsyncDriver, two source buffers, and sentinel destination bytes.
    let (runner, worker) = AsyncDevice::new(
        pdma_model_device::PdmaMemoryProvisioning::shared(std::sync::Arc::new(
            hardware_interface_runtime::SharedMemory::new(),
        )),
        pdma_model_contract::semantic::ResourceConfig::default(),
    );
    let worker = tokio::spawn(worker.run());
    let (snapshot, completion, command_id, no_completion, dst_bytes) = SoftwareCpu::new()
        .run_main(async {
            let mut handle = PdmaRegisterPort::new(AsyncDeviceAccess::new(
                runner.handle(),
                pdma_model_host::adapter::abi::PdmaIrqCoordinator::default(),
            ))
            .expect("active CPU creates register port");
            let mut driver = AsyncDriver::setup_on(&mut handle, 4, 4)
                .await
                .expect("queue setup is valid");
            let src_a = driver
                .allocate_buffer_on(&mut handle, 12)
                .await
                .expect("allocation");
            let src_b = driver
                .allocate_buffer_on(&mut handle, 12)
                .await
                .expect("allocation");
            let dst = driver
                .allocate_buffer_on(&mut handle, 16)
                .await
                .expect("allocation");
            driver
                .write_buffer_on(
                    &mut handle,
                    src_a,
                    &[
                        0xA0, 0xA1, 1, 250, 0, 127, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7,
                    ],
                )
                .await
                .expect("source A write is in bounds");
            driver
                .write_buffer_on(
                    &mut handle,
                    src_b,
                    &[
                        0xB0, 0xB1, 2, 10, 255, 1, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7,
                    ],
                )
                .await
                .expect("source B write is in bounds");
            driver
                .write_buffer_on(&mut handle, dst, &[0xCC; 16])
                .await
                .expect("destination write is in bounds");

            // When: the software main submits vector-add, waits for progress, and polls completion.
            let command_id = driver
                .submit_vector_add_on(
                    &mut handle,
                    VectorAddRequest::new(
                        VectorBufferRange::new(src_a, 2),
                        VectorBufferRange::new(src_b, 2),
                        VectorBufferRange::new(dst, 6),
                        4,
                    ),
                )
                .await
                .expect("vector-add submission has ring space");
            let snapshot = runner.wait_for_progress_after(0).await;
            let completion = driver
                .poll_completion_on(&mut handle)
                .await
                .expect("completion ring exists")
                .expect("vector-add published one completion");
            let no_completion = driver.poll_completion_on(&mut handle).await;
            let dst_bytes = driver.read_buffer_on(&handle, dst, 16).await;
            (snapshot, completion, command_id, no_completion, dst_bytes)
        })
        .await;
    runner.request_shutdown().await;
    let summary = worker.await.expect("runner joins cleanly");

    // Then: byte-wise wrapping addition lands at the destination offset with one completion.
    assert_eq!(snapshot.steps, 1);
    assert_eq!(completion.command_id(), command_id);
    assert_eq!(completion.status(), CompletionStatus::Success);
    assert_eq!(no_completion, Ok(None));
    assert_eq!(
        dst_bytes,
        Ok(vec![
            0xCC, 0xCC, 0xCC, 0xCC, 0xCC, 0xCC, 3, 4, 255, 128, 0xCC, 0xCC, 0xCC, 0xCC, 0xCC, 0xCC,
        ])
    );
    assert_eq!(summary.steps, 1);
}

#[tokio::test(flavor = "current_thread")]
async fn async_runner_publishes_event_and_no_completion_when_vector_add_source_b_read_fails() {
    // Given: async runner handle, AsyncDriver, and a vector add whose source B read is too long.
    let (runner, worker) = AsyncDevice::new(
        pdma_model_device::PdmaMemoryProvisioning::shared(std::sync::Arc::new(
            hardware_interface_runtime::SharedMemory::new(),
        )),
        pdma_model_contract::semantic::ResourceConfig::default(),
    );
    let worker = tokio::spawn(worker.run());
    let (command_id, snapshot, event, src_b, no_completion) = SoftwareCpu::new()
        .run_main(async {
            let mut handle = PdmaRegisterPort::new(AsyncDeviceAccess::new(
                runner.handle(),
                pdma_model_host::adapter::abi::PdmaIrqCoordinator::default(),
            ))
            .expect("active CPU creates register port");
            let mut driver = AsyncDriver::setup_on(&mut handle, 4, 4)
                .await
                .expect("queue setup is valid");
            let src_a = driver
                .allocate_buffer_on(&mut handle, 8)
                .await
                .expect("allocation");
            let src_b = driver
                .allocate_buffer_on(&mut handle, 4)
                .await
                .expect("allocation");
            let dst = driver
                .allocate_buffer_on(&mut handle, 8)
                .await
                .expect("allocation");

            // When: the invalid vector add is submitted and the async runner executes it.
            let command_id = driver
                .submit_vector_add_on(
                    &mut handle,
                    VectorAddRequest::new(
                        VectorBufferRange::new(src_a, 1),
                        VectorBufferRange::new(src_b, 1),
                        VectorBufferRange::new(dst, 0),
                        4,
                    ),
                )
                .await
                .expect("vector-add submission has ring space");
            let snapshot = runner.wait_for_error_after(0).await;
            let event = driver
                .poll_event_on(&mut handle)
                .await
                .expect("event ring exists")
                .expect("source-B fault event was published");
            let no_completion = driver.poll_completion_on(&mut handle).await;
            (command_id, snapshot, event, src_b, no_completion)
        })
        .await;
    runner.request_shutdown().await;
    let summary = worker.await.expect("runner joins cleanly");

    // Then: the original out-of-bounds event appears and no success completion appears.
    assert_eq!(command_id.raw(), 0);
    assert_eq!(snapshot.errors, 1);
    assert_eq!(event.command_id(), command_id);
    assert_eq!(event.buffer(), src_b);
    assert_eq!(
        event.error_kind(),
        EventErrorKind::OutOfBounds { len: 4, end: 5 }
    );
    assert_eq!(no_completion, Ok(None));
    assert_eq!(summary.errors, 1);
}

#[tokio::test(flavor = "current_thread")]
async fn async_runner_publishes_event_and_no_completion_when_vector_add_destination_write_fails() {
    // Given: async runner handle, AsyncDriver, and a vector add whose destination write is too long.
    let (runner, worker) = AsyncDevice::new(
        pdma_model_device::PdmaMemoryProvisioning::shared(std::sync::Arc::new(
            hardware_interface_runtime::SharedMemory::new(),
        )),
        pdma_model_contract::semantic::ResourceConfig::default(),
    );
    let worker = tokio::spawn(worker.run());
    let (command_id, snapshot, event, dst, no_completion) = SoftwareCpu::new()
        .run_main(async {
            let mut handle = PdmaRegisterPort::new(AsyncDeviceAccess::new(
                runner.handle(),
                pdma_model_host::adapter::abi::PdmaIrqCoordinator::default(),
            ))
            .expect("active CPU creates register port");
            let mut driver = AsyncDriver::setup_on(&mut handle, 4, 4)
                .await
                .expect("queue setup is valid");
            let src_a = driver
                .allocate_buffer_on(&mut handle, 8)
                .await
                .expect("allocation");
            let src_b = driver
                .allocate_buffer_on(&mut handle, 8)
                .await
                .expect("allocation");
            let dst = driver
                .allocate_buffer_on(&mut handle, 4)
                .await
                .expect("allocation");
            driver
                .write_buffer_on(&mut handle, src_a, &[0xD0, 0xD1, 9, 8, 7, 6, 0xD2, 0xD3])
                .await
                .expect("source A write is in bounds");
            driver
                .write_buffer_on(&mut handle, src_b, &[0xE0, 0xE1, 1, 2, 3, 4, 0xE2, 0xE3])
                .await
                .expect("source B write is in bounds");

            // When: the vector add tries to write four bytes starting before the end of the destination.
            let command_id = driver
                .submit_vector_add_on(
                    &mut handle,
                    VectorAddRequest::new(
                        VectorBufferRange::new(src_a, 2),
                        VectorBufferRange::new(src_b, 2),
                        VectorBufferRange::new(dst, 1),
                        4,
                    ),
                )
                .await
                .expect("vector-add submission has ring space");
            let snapshot = runner.wait_for_error_after(0).await;
            let event = driver
                .poll_event_on(&mut handle)
                .await
                .expect("event ring exists")
                .expect("destination fault event was published");
            let no_completion = driver.poll_completion_on(&mut handle).await;
            (command_id, snapshot, event, dst, no_completion)
        })
        .await;
    runner.request_shutdown().await;
    let summary = worker.await.expect("runner joins cleanly");

    // Then: the destination bounds event appears and no success completion appears.
    assert_eq!(command_id.raw(), 0);
    assert_eq!(snapshot.errors, 1);
    assert_eq!(event.command_id(), command_id);
    assert_eq!(event.buffer(), dst);
    assert_eq!(
        event.error_kind(),
        EventErrorKind::OutOfBounds { len: 4, end: 5 }
    );
    assert_eq!(no_completion, Ok(None));
    assert_eq!(summary.errors, 1);
}
