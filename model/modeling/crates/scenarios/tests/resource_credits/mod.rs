#![allow(missing_docs)]

mod vector_read;

use hardware_interface_runtime::SoftwareCpu;
use pdma_model_contract::semantic::{CompletionStatus, ResourceConfig};
use pdma_model_device::AsyncDevice;
use pdma_model_host::{
    AsyncDriver,
    adapter::abi::{AsyncDeviceAccess, PdmaRegisterPort},
};
use pdma_model_host::{
    BroadcastCopyRequest, BroadcastDestinationRange, CopyRequest, VectorAddRequest,
    VectorBufferRange,
};

#[tokio::test(flavor = "current_thread")]
async fn async_runner_vector_add_stalls_with_one_read_slot() {
    vector_read::run_vector_add_one_read_slot_scenario().await;
}

#[tokio::test(flavor = "current_thread")]
async fn async_runner_copy_stalls_without_read_slots() {
    // Given: an async copy on a device with no read slots.
    let host_memory = pdma_model_host::adapter::abi::PdmaHostMemory::new(std::sync::Arc::new(
        hardware_interface_runtime::SharedMemory::new(),
    ));
    let (runner, worker) = AsyncDevice::new(
        host_memory.memory_access(),
        ResourceConfig::default().with_read_slots(0),
    );
    let worker = tokio::spawn(worker.run());
    let (command_id, snapshot, completion) = SoftwareCpu::new()
        .run_main(async {
            let mut handle =
                PdmaRegisterPort::new(AsyncDeviceAccess::new(runner.handle(), host_memory.clone()))
                    .expect("active software CPU");
            let mut driver = AsyncDriver::setup_on(&mut handle, 4, 4)
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
            let mut snapshot = runner.snapshot().await;

            // When: software submits the copy and the async runner tries to retire it.
            let command_id = driver
                .submit_copy_on(&mut handle, CopyRequest::new(src, dst, 4))
                .await
                .expect("copy submission has ring space");
            while snapshot.stalls == 0 {
                snapshot = runner.wait_for_change_after(snapshot).await;
            }
            let completion = driver.poll_completion_on(&mut handle).await;
            (command_id, snapshot, completion)
        })
        .await;
    runner.request_shutdown().await;
    let summary = worker.await.expect("runner joins cleanly");

    // Then: the command stays pending because the read slot budget is empty.
    assert_eq!(command_id.raw(), 0);
    assert_eq!(snapshot.steps, 0);
    assert_eq!(snapshot.errors, 0);
    assert_eq!(snapshot.stalls, 1);
    assert_eq!(summary.steps, 0);
    assert_eq!(summary.errors, 0);
    assert_eq!(summary.stalls, 1);
    assert_eq!(completion, Ok(None));
}

#[tokio::test(flavor = "current_thread")]
async fn async_runner_copy_completes_with_one_read_slot() {
    // Given: an async copy on a device with one read slot available.
    let host_memory = pdma_model_host::adapter::abi::PdmaHostMemory::new(std::sync::Arc::new(
        hardware_interface_runtime::SharedMemory::new(),
    ));
    let (runner, worker) = AsyncDevice::new(
        host_memory.memory_access(),
        ResourceConfig::default().with_read_slots(1),
    );
    let worker = tokio::spawn(worker.run());
    let (command_id, snapshot, completion, copied) = SoftwareCpu::new()
        .run_main(async {
            let mut handle =
                PdmaRegisterPort::new(AsyncDeviceAccess::new(runner.handle(), host_memory.clone()))
                    .expect("active software CPU");
            let mut driver = AsyncDriver::setup_on(&mut handle, 4, 4)
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

            // When: software submits the copy and the async runner retires it.
            let command_id = driver
                .submit_copy_on(&mut handle, CopyRequest::new(src, dst, 4))
                .await
                .expect("copy submission has ring space");
            let snapshot = runner.wait_for_progress_after(0).await;
            let completion = driver
                .poll_completion_on(&mut handle)
                .await
                .expect("completion ring exists")
                .expect("copy published one completion");
            let copied = driver.read_buffer_on(&handle, dst, 4).await;
            (command_id, snapshot, completion, copied)
        })
        .await;
    runner.request_shutdown().await;
    let summary = worker.await.expect("runner joins cleanly");

    // Then: the copy completes successfully and the destination matches the source.
    assert_eq!(snapshot.steps, 1);
    assert_eq!(summary.steps, 1);
    assert_eq!(completion.command_id(), command_id);
    assert_eq!(completion.status(), CompletionStatus::Success);
    assert_eq!(copied, Ok(vec![1, 2, 3, 4]));
}

#[tokio::test(flavor = "current_thread")]
async fn async_runner_broadcast_stalls_with_insufficient_write_and_descriptor_credit() {
    // Given: an async broadcast on a device with insufficient write and descriptor credit.
    let host_memory = pdma_model_host::adapter::abi::PdmaHostMemory::new(std::sync::Arc::new(
        hardware_interface_runtime::SharedMemory::new(),
    ));
    let (runner, worker) = AsyncDevice::new(
        host_memory.memory_access(),
        ResourceConfig::default()
            .with_write_slots(1)
            .with_internal_descriptors(1),
    );
    let worker = tokio::spawn(worker.run());
    let (command_id, snapshot, completion) = SoftwareCpu::new()
        .run_main(async {
            let mut handle =
                PdmaRegisterPort::new(AsyncDeviceAccess::new(runner.handle(), host_memory.clone()))
                    .expect("active software CPU");
            let mut driver = AsyncDriver::setup_on(&mut handle, 4, 4)
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
            let mut snapshot = runner.snapshot().await;

            // When: software submits one broadcast command and the async runner tries to retire it.
            let command_id = driver
                .submit_broadcast_copy_on(
                    &mut handle,
                    BroadcastCopyRequest::new(
                        src,
                        2,
                        4,
                        vec![
                            BroadcastDestinationRange::new(dst_a, 0),
                            BroadcastDestinationRange::new(dst_b, 0),
                        ],
                    ),
                )
                .await
                .expect("broadcast submission has ring space");
            while snapshot.stalls == 0 {
                snapshot = runner.wait_for_change_after(snapshot).await;
            }
            let completion = driver.poll_completion_on(&mut handle).await;
            (command_id, snapshot, completion)
        })
        .await;
    runner.request_shutdown().await;
    let summary = worker.await.expect("runner joins cleanly");

    // Then: the command stays pending instead of retiring with too little credit.
    assert_eq!(command_id.raw(), 0);
    assert_eq!(snapshot.steps, 0);
    assert_eq!(snapshot.errors, 0);
    assert_eq!(snapshot.stalls, 1);
    assert_eq!(summary.steps, 0);
    assert_eq!(summary.errors, 0);
    assert_eq!(summary.stalls, 1);
    assert_eq!(completion, Ok(None));
}

mod compute;
