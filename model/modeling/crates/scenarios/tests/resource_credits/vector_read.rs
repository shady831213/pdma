use super::*;

#[allow(clippy::expect_used, clippy::redundant_pub_crate)]
pub(super) async fn run_vector_add_one_read_slot_scenario() {
    // Given: an async vector-add command on a device with only one read slot.
    let (runner, worker) = AsyncDevice::new(
        pdma_model_device::PdmaMemoryProvisioning::shared(std::sync::Arc::new(
            hardware_interface_runtime::SharedMemory::new(),
        )),
        ResourceConfig::default().with_read_slots(1),
    );
    let worker = tokio::spawn(worker.run());
    let (command_id, snapshot, completion) = SoftwareCpu::new()
        .run_main(async {
            let mut handle = PdmaRegisterPort::new(AsyncDeviceAccess::new(runner.handle()))
                .expect("active software CPU");
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
                .allocate_buffer_on(&mut handle, 8)
                .await
                .expect("allocation");
            let mut snapshot = runner.snapshot().await;

            // When: software submits the vector add and the async runner tries to retire it.
            let command_id = driver
                .submit_vector_add_on(
                    &mut handle,
                    VectorAddRequest::new(
                        VectorBufferRange::new(src_a, 0),
                        VectorBufferRange::new(src_b, 0),
                        VectorBufferRange::new(dst, 0),
                        4,
                    ),
                )
                .await
                .expect("vector-add submission has ring space");
            while snapshot.stalls == 0 {
                snapshot = runner.wait_for_change_after(snapshot).await;
            }
            let completion = driver.poll_completion_on(&mut handle).await;
            (command_id, snapshot, completion)
        })
        .await;
    runner.request_shutdown().await;
    let summary = worker.await.expect("runner joins cleanly");

    // Then: the command stays pending because vector-add needs two read slots.
    assert_eq!(command_id.raw(), 0);
    assert_eq!(snapshot.steps, 0);
    assert_eq!(snapshot.errors, 0);
    assert_eq!(snapshot.stalls, 1);
    assert_eq!(summary.steps, 0);
    assert_eq!(summary.errors, 0);
    assert_eq!(summary.stalls, 1);
    assert_eq!(completion, Ok(None));
}
