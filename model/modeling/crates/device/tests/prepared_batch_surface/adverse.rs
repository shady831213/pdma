use super::*;

#[path = "adverse/writer.rs"]
mod writer;

pub(super) async fn run() -> support::TestResult {
    // Given: independent stale and active-queue fixtures with full public pre-state snapshots.
    let (handle, observer) = support::observed_handle();
    let stale = handle.stage_memory_regions(&[128, 64, 128]).await?;
    let activation = support::activation(&stale, DeviceEpoch::initial())?;
    let _competing_generation = handle.allocate_memory_region(1).await?;
    let snapshot = handle.snapshot().await;
    let generation = handle.memory_generation().await;
    let registers = support::queue_latches(&handle).await?;
    let trace = handle.trace_events().await;
    let mut stale_wake = Box::pin(handle.wait_for_change_after(snapshot));
    assert!(poll_once_pending(&mut stale_wake).await);
    let (active, active_observer) = support::observed_handle();
    let (queue, submission, _) = support::bootstrap(&active).await?;
    let active_snapshot = active.snapshot().await;
    let active_generation = active.memory_generation().await;
    let active_registers = support::queue_latches(&active).await?;
    let active_observations = active_observer.snapshot().len();
    let active_trace = active.trace_events().await;
    let mut active_wake = Box::pin(active.wait_for_change_after(active_snapshot));
    assert!(poll_once_pending(&mut active_wake).await);

    // When: all publicly constructible adverse operations execute through real surfaces.
    let stale = handle
        .bootstrap_queue_batch(QueueBootstrapBatch::new(stale, activation))
        .await
        .is_err();
    let [capacity, overlap] = capacity_and_overlap().await?;
    let ([length, range, queue_failure, decode, access, mask], cancelled_region) =
        exercise_active(&active, queue, submission).await?;
    let response = response_encoding_limit();
    let wakes_zero =
        poll_once_pending(&mut stale_wake).await && poll_once_pending(&mut active_wake).await;
    assert!(wakes_zero);
    drop(stale_wake);
    drop(active_wake);
    let writer_serialized = writer::run().await?;
    assert!(writer_serialized);

    // Then: every failure and cancellation leaves all captured public surfaces unchanged.
    assert!(
        [
            stale,
            capacity,
            overlap,
            length,
            range,
            queue_failure,
            decode,
            access,
            mask,
            response,
        ]
        .into_iter()
        .all(core::convert::identity)
    );
    assert_eq!(handle.snapshot().await, snapshot);
    assert_eq!(handle.memory_generation().await, generation);
    assert_eq!(support::queue_latches(&handle).await?, registers);
    assert_eq!(handle.trace_events().await, trace);
    assert!(observer.snapshot().is_empty());
    assert_eq!(active.snapshot().await, active_snapshot);
    assert_eq!(active.memory_generation().await, active_generation);
    assert_eq!(support::queue_latches(&active).await?, active_registers);
    assert_eq!(active_observer.snapshot().len(), active_observations);
    assert_eq!(active.trace_events().await, active_trace);
    assert_eq!(
        active.region_for_object(cancelled_region.object()).await,
        None
    );
    println!(
        "failures=stale,capacity,overlap,range,length,decode,access,mask,queue,response;observations=0;wakes={};bytes_unchanged=true;mappings_unchanged=true;generation_unchanged=true;registers_unchanged=true;control_unchanged=true;trace_unchanged=true;cancelled_unchanged=true;writer_serialized={writer_serialized}",
        usize::from(!wakes_zero),
    );
    Ok(())
}

fn response_encoding_limit() -> bool {
    let mut short = [0_u8; 71];
    let rejected =
        pdma_model_contract::abi::mmio::QueueResponseRegistersViewMut::new(&mut short).is_err();
    assert!(rejected);
    rejected
}

async fn poll_once_pending<F: std::future::Future>(future: &mut std::pin::Pin<Box<F>>) -> bool {
    std::future::poll_fn(|context| {
        std::task::Poll::Ready(future.as_mut().poll(context).is_pending())
    })
    .await
}

async fn capacity_and_overlap() -> support::TestResult<[bool; 2]> {
    let capacity_memory =
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::with_region_capacity(2));
    let (capacity_handle, capacity_observer) =
        support::observed_handle_for_memory(&capacity_memory);
    let capacity_stage = capacity_handle
        .stage_memory_regions(&[128, 64, 128])
        .await?;
    let capacity_activation = support::activation(&capacity_stage, DeviceEpoch::initial())?;
    let capacity = capacity_handle
        .bootstrap_queue_batch(QueueBootstrapBatch::new(
            capacity_stage,
            capacity_activation,
        ))
        .await
        .is_err();
    assert!(capacity_memory.regions().is_empty());
    assert_eq!(capacity_handle.memory_generation().await, 0);
    assert!(capacity_observer.snapshot().is_empty());

    let overlap_memory = std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new());
    let overlapping = MemoryRegion::try_new(
        MemoryAddress::new(0x1_0000),
        64,
        MemoryAlignment::try_new(64)?,
    )?;
    overlap_memory
        .prepare_regions(&[overlapping])?
        .try_install(&overlap_memory)?;
    let overlap_revision = overlap_memory.revision();
    let (overlap_handle, overlap_observer) = support::observed_handle_for_memory(&overlap_memory);
    let overlap_stage = overlap_handle.stage_memory_regions(&[128, 64, 128]).await?;
    let overlap_activation = support::activation(&overlap_stage, DeviceEpoch::initial())?;
    let overlap = overlap_handle
        .bootstrap_queue_batch(QueueBootstrapBatch::new(overlap_stage, overlap_activation))
        .await
        .is_err();
    assert_eq!(overlap_memory.revision(), overlap_revision);
    assert_eq!(overlap_memory.regions(), vec![overlapping]);
    assert_eq!(overlap_handle.memory_generation().await, 0);
    assert!(overlap_observer.snapshot().is_empty());
    Ok([capacity, overlap])
}

async fn exercise_active(
    active: &pdma_model_device::AsyncDeviceHandle,
    queue: pdma_model_contract::EpochQueue,
    submission: pdma_model_contract::PdmaMemoryRegion,
) -> support::TestResult<([bool; 6], pdma_model_contract::PdmaMemoryRegion)> {
    let length = active
        .submit_queue_batch(SubmissionBatch::new(
            queue,
            RingCursor::new(0),
            DmaWrite::new(submission.base(), vec![0; 63]),
            None,
        ))
        .await
        .is_err();
    let range = active
        .submit_queue_batch(SubmissionBatch::new(
            queue,
            RingCursor::new(0),
            DmaWrite::new(DmaAddress::new(0xdead_0000), vec![0; 64]),
            None,
        ))
        .await
        .is_err();
    let queue_failure = active
        .submit_queue_batch(support::submission(
            queue,
            RingCursor::new(1),
            submission.base(),
            0xaa,
        ))
        .await
        .is_err();
    let endpoint = active.register_endpoint();
    let mut malformed = [0_u8; 1];
    let decode = endpoint.read(0, &mut malformed).await.is_err();
    let access = endpoint.write(0, &[0; 4]).await.is_err();
    let mask = endpoint
        .write(0x008, &u32::MAX.to_le_bytes())
        .await
        .is_err();
    let cancelled_stage = active.stage_memory_regions(&[8]).await?;
    let cancelled_region = cancelled_stage
        .regions()
        .first()
        .copied()
        .ok_or("one cancelled region required")?;
    let cancelled = active.submit_queue_batch(SubmissionBatch::new(
        queue,
        RingCursor::new(0),
        DmaWrite::new(submission.base(), vec![0; 64]),
        Some((
            cancelled_stage,
            DmaWrite::new(cancelled_region.base(), vec![0; 8]),
        )),
    ));
    drop(cancelled);
    Ok((
        [length, range, queue_failure, decode, access, mask],
        cancelled_region,
    ))
}
