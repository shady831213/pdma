use crate::*;

#[tokio::test(flavor = "current_thread")]
async fn async_runner_surface_reports_a_shutdown_and_join_summary() {
    // Given: an async runner and one completed command.
    let host_memory = pdma_model_host::adapter::abi::PdmaHostMemory::new(std::sync::Arc::new(
        hardware_interface_runtime::SharedMemory::new(),
    ));
    let (runner, worker) = AsyncDevice::new(
        host_memory.memory_access(),
        pdma_model_contract::semantic::ResourceConfig::default(),
    );
    let worker = tokio::spawn(worker.run());
    let (command_id, completion, snapshot, copied) = SoftwareCpu::new()
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
                .write_buffer_on(&mut handle, src, &[1, 3, 5, 7])
                .await
                .expect("source write is in bounds");

            // When: software submits a copy, waits once, and then shuts the async runner down.
            let command_id = driver
                .submit_copy_on(&mut handle, CopyRequest::new(src, dst, 4))
                .await
                .expect("copy submission has ring space");
            let snapshot = runner.wait_for_progress_after(0).await;
            let completion = driver
                .poll_completion_on(&mut handle)
                .await
                .expect("completion ring exists")
                .expect("completion was published");
            let copied = driver.read_buffer_on(&handle, dst, 4).await;
            (command_id, completion, snapshot, copied)
        })
        .await;
    runner.request_shutdown().await;
    let summary = worker.await.expect("runner joins cleanly");

    // Then: shutdown produces a join summary after the successful copy.
    assert_eq!(completion.command_id(), command_id);
    assert_eq!(completion.status(), CompletionStatus::Success);
    assert_eq!(snapshot.steps, 1);
    assert_eq!(summary.steps, 1);
    assert_eq!(copied, Ok(vec![1, 3, 5, 7]));
}
