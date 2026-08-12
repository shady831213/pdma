use super::support::BlockingPort;
use pdma_model_contract::{CommandId, CopyRequest};
use pdma_model_device::AsyncDevice;
use pdma_model_host::ModelError;
use pdma_model_host::ModelResult;
use pdma_model_host::{AsyncDriver, SoftwareCpu};
use tokio::sync::oneshot;

async fn setup_copy(port: &mut BlockingPort) -> ModelResult<(AsyncDriver, CopyRequest)> {
    let driver = AsyncDriver::setup_on(port, 1, 1).await?;
    let src = driver.allocate_buffer_on(port, 4).await?;
    let dst = driver.allocate_buffer_on(port, 4).await?;
    Ok((driver, CopyRequest::new(src, dst, 4)))
}

#[tokio::test(flavor = "current_thread")]
async fn submit_failure_restores_slot_but_consumes_command_id() {
    // Given: a depth-one driver whose first device submission fails before acceptance.
    let (device, _worker) = AsyncDevice::queue_port();
    let mut port = BlockingPort::new(device.handle());
    port.fail_next_submit();

    // When: software retries through the same accounting capability.
    let (failed, retried) = SoftwareCpu::new()
        .run_main(async {
            let (mut driver, request) = setup_copy(&mut port).await?;
            let failed = driver.submit_copy_on(&mut port, request).await;
            let retried = driver.submit_copy_on(&mut port, request).await;
            Ok::<_, ModelError>((failed, retried))
        })
        .await
        .expect("setup remains valid");

    // Then: the credit returns while the failed command ID remains consumed.
    assert_eq!(failed, Err(ModelError::RingFull));
    assert_eq!(retried, Ok(CommandId::new(1)));
}

#[tokio::test(flavor = "current_thread")]
async fn inner_select_cancellation_restores_slot_but_consumes_command_id() {
    // Given: a submit future blocked before device acceptance inside one complete main.
    let (device, _worker) = AsyncDevice::queue_port();
    let mut port = BlockingPort::new(device.handle());
    let control = port.clone();
    let observer = control.clone();
    control.block_submit();
    let (cancel_tx, cancel_rx) = oneshot::channel();
    let cpu = SoftwareCpu::new();
    let main = tokio::spawn(async move {
        cpu.run_main(async move {
            let (mut driver, request) = setup_copy(&mut port).await?;
            {
                let submit = driver.submit_copy_on(&mut port, request);
                tokio::pin!(submit);
                tokio::select! {
                    biased;
                    result = &mut submit => return result.map(|_| CommandId::new(usize::MAX)),
                    result = cancel_rx => result.map_err(|_| ModelError::RingFull)?,
                }
            }
            control.allow_submit();
            driver.submit_copy_on(&mut port, request).await
        })
        .await
    });

    // When: the selected cancellation branch drops the blocked submit future.
    observer.wait_submit_entered().await;
    cancel_tx
        .send(())
        .expect("main still waits for cancellation");
    let retried = main
        .await
        .expect("main task joins")
        .expect("retry succeeds after rollback");

    // Then: the next submission uses restored credit and the next command ID.
    assert_eq!(retried, CommandId::new(1));
}

#[tokio::test(flavor = "current_thread")]
async fn aborted_run_main_restores_slot_for_retained_clone() {
    // Given: main publishes a clone before blocking a submission ahead of acceptance.
    let (device, _worker) = AsyncDevice::queue_port();
    let mut main_port = BlockingPort::new(device.handle());
    main_port.block_submit();
    let observer = main_port.clone();
    let mut retry_port = main_port.clone();
    let cpu = SoftwareCpu::new();
    let task_cpu = cpu.clone();
    let (retained_tx, retained_rx) = oneshot::channel();
    let main = tokio::spawn(async move {
        task_cpu
            .run_main(async move {
                let (mut driver, request) = setup_copy(&mut main_port).await?;
                retained_tx
                    .send((driver.clone(), request))
                    .map_err(|_| ModelError::RingFull)?;
                driver.submit_copy_on(&mut main_port, request).await
            })
            .await
    });
    let (mut retained, request) = retained_rx
        .await
        .expect("main publishes retained capability");
    observer.wait_submit_entered().await;

    // When: the whole run_main task is aborted while the reservation is armed.
    main.abort();
    assert!(main.await.expect_err("main is cancelled").is_cancelled());
    observer.allow_submit();
    let retried = cpu
        .run_main(retained.submit_copy_on(&mut retry_port, request))
        .await;

    // Then: Drop queued a rollback without needing active CPU state.
    assert_eq!(retried, Ok(CommandId::new(1)));
}

#[tokio::test(flavor = "current_thread")]
async fn doorbell_failure_keeps_accepted_command_slot_consumed() {
    // Given: a depth-one queue whose first doorbell fails after command acceptance.
    let (device, _worker) = AsyncDevice::queue_port();
    let mut port = BlockingPort::new(device.handle());
    port.fail_next_doorbell();

    // When: software attempts another submission through the same driver.
    let (failed, second) = SoftwareCpu::new()
        .run_main(async {
            let (mut driver, request) = setup_copy(&mut port).await?;
            let failed = driver.submit_copy_on(&mut port, request).await;
            let second = driver.submit_copy_on(&mut port, request).await;
            Ok::<_, ModelError>((failed, second))
        })
        .await
        .expect("setup remains valid");

    // Then: acceptance commits the slot before the doorbell await.
    assert_eq!(failed, Err(ModelError::RingFull));
    assert_eq!(second, Err(ModelError::DriverQueueFull));
}

#[tokio::test(flavor = "current_thread")]
async fn cancelled_doorbell_keeps_accepted_command_slot_consumed() {
    // Given: a submit blocked at the doorbell after device acceptance.
    let (device, _worker) = AsyncDevice::queue_port();
    let mut port = BlockingPort::new(device.handle());
    let observer = port.clone();
    observer.block_doorbell();
    let (cancel_tx, cancel_rx) = oneshot::channel();
    let cpu = SoftwareCpu::new();
    let main = tokio::spawn(async move {
        cpu.run_main(async move {
            let (mut driver, request) = setup_copy(&mut port).await?;
            {
                let submit = driver.submit_copy_on(&mut port, request);
                tokio::pin!(submit);
                tokio::select! {
                    biased;
                    result = &mut submit => return result.map(|_| Err(ModelError::RingFull)),
                    result = cancel_rx => result.map_err(|_| ModelError::RingFull)?,
                }
            }
            Ok::<_, ModelError>(driver.submit_copy_on(&mut port, request).await)
        })
        .await
    });
    observer.wait_doorbell_entered().await;

    // When: cancellation drops the future after its reservation was disarmed.
    cancel_tx
        .send(())
        .expect("main still waits for cancellation");
    let second = main
        .await
        .expect("main joins")
        .expect("setup and cancellation are valid");
    observer.allow_doorbell();

    // Then: no rollback is queued for a device-accepted command.
    assert_eq!(second, Err(ModelError::DriverQueueFull));
}
