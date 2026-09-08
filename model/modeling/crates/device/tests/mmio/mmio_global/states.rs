use super::*;
use pdma_model_contract::abi::registers::QueueRegisters;

#[tokio::test(flavor = "current_thread")]
async fn status_distinguishes_all_current_work_and_pressure_classes_without_mutation() -> TestResult
{
    for (config, work, expected) in [
        (ResourceConfig::default(), false, 0),
        (ResourceConfig::default(), true, 1),
        (ResourceConfig::default().with_read_slots(0), true, 2),
    ] {
        // Given: idle, runnable, or resource-blocked work with the worker unstarted.
        let (device, _worker) = device(config);
        let handle = device.handle();
        let endpoint = handle.register_endpoint();
        let [submission, _, _] = create_queue(&handle, 2).await?;
        let target = if work {
            let target = submit_copy(&handle, submission.base(), 0).await?;
            write_u64(&endpoint, q(QueueRegisters::DOORBELL.offset())?, 1).await?;
            Some(target)
        } else {
            None
        };
        let before = handle.snapshot().await;
        let memory = handle.memory_generation().await;
        let queues = cursors(&endpoint).await?;
        let irq = handle.irq_snapshot().await;
        // When: repeated direct run-state reads observe the work.
        for _ in 0..2 {
            assert_eq!(
                read_u32(
                    &endpoint,
                    u32::try_from(GlobalRegisters::RUN_STATE.offset())?
                )
                .await?,
                expected
            );
        }
        // Then: lifecycle, DMA, queues and IRQ remain unchanged.
        assert_eq!(handle.snapshot().await, before);
        assert_eq!(handle.memory_generation().await, memory);
        assert_eq!(cursors(&endpoint).await?, queues);
        assert_eq!(handle.irq_snapshot().await, irq);
        if let Some(target) = target {
            assert_eq!(handle.read_memory(target.base(), 4).await?, [0; 4]);
        }
    }
    Ok(())
}

#[tokio::test(flavor = "current_thread")]
async fn status_observes_real_completion_and_event_publication_pressure() -> TestResult {
    for (config, fault) in [
        (
            ResourceConfig::default().with_completion_publication_slots(0),
            false,
        ),
        (
            ResourceConfig::default().with_event_publication_slots(0),
            true,
        ),
    ] {
        // Given: work whose result cannot be published with configured credits.
        let (device, worker) = device(config);
        let handle = device.handle();
        let endpoint = handle.register_endpoint();
        let [submission, _, _] = create_queue(&handle, 2).await?;
        if fault {
            handle.write_memory(submission.base(), &[0; 64]).await?;
            write_u64(&endpoint, q(QueueRegisters::SQ_PRODUCER.offset())?, 1).await?;
        } else {
            submit_copy(&handle, submission.base(), 0).await?;
        }
        write_u64(&endpoint, q(QueueRegisters::DOORBELL.offset())?, 1).await?;
        let worker = tokio::spawn(worker.run());
        let stalled = device.wait_for_stall_after(0).await;
        // When: software reads the live status after the actual worker stall.
        let state = read_u32(
            &endpoint,
            u32::try_from(GlobalRegisters::RUN_STATE.offset())?,
        )
        .await?;
        // Then: status is blocked and the read takes no additional step.
        assert_eq!(state, 2);
        assert_eq!(handle.snapshot().await, stalled);
        device.request_shutdown().await;
        worker.await?;
    }
    Ok(())
}
