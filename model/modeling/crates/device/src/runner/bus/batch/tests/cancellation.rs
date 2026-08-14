use super::*;
use crate::runner::bus::{DmaWrite, SubmissionBatch};
use pdma_model_contract::abi::RingCursor;
use std::future::{Future, poll_fn};

#[tokio::test]
async fn bootstrap_and_submission_pending_lock_drops_are_mutation_free() -> TestResult {
    // Given: one active queue and complete public/private state snapshots.
    let memory = Arc::new(hardware_interface_runtime::SharedMemory::new());
    let observer = Arc::new(CommitOrderObserver {
        memory: Some(Arc::clone(&memory)),
        expected_region_count: std::sync::atomic::AtomicUsize::new(3),
        ..CommitOrderObserver::default()
    });
    let (device, _worker) = AsyncDevice::from_core(crate::DeviceCore::new(
        Arc::clone(&memory),
        pdma_model_contract::ResourceConfig::default(),
        observer.clone(),
    ));
    let handle = device.handle();
    let active_stage = handle.stage_memory_regions(&[128, 64, 128]).await?;
    let [descriptor, _, _] = active_stage.regions() else {
        return Err("three staged regions required".into());
    };
    let descriptor = *descriptor;
    let active = activation(&active_stage)?;
    handle
        .bootstrap_queue_batch(QueueBootstrapBatch::new(active_stage, active))
        .await?;

    // When: each operation is genuinely polled Pending on the runner lock, then dropped.
    for iteration in 0..25 {
        let bootstrap_stage = handle.stage_memory_regions(&[128, 64, 128]).await?;
        let bootstrap = activation(&bootstrap_stage)?;
        let indirect = handle.stage_memory_regions(&[8]).await?;
        let [indirect_region] = indirect.regions() else {
            return Err("one staged indirect region required".into());
        };
        let indirect_region = *indirect_region;
        let state = handle.shared.state.lock().await;
        let before_memory = memory_image(&memory)?;
        let before_mapping_generation = state.dma_space.generation();
        let before_runner_generation = state.generation;
        let before_wake_generation = state.wake_generation;
        let before_latches = queue_latch_values(&state);
        let before_trace = state.backend.trace_events().clone();
        let before_observations = observer
            .observations
            .lock()
            .expect("observation probe lock available")
            .clone();

        let mut bootstrap_future = Box::pin(
            handle.bootstrap_queue_batch(QueueBootstrapBatch::new(bootstrap_stage, bootstrap)),
        );
        assert!(poll_once_pending(&mut bootstrap_future).await);
        drop(bootstrap_future);
        let mut submission_future = Box::pin(handle.submit_queue_batch(SubmissionBatch::new(
            active.queue(),
            RingCursor::new(0),
            DmaWrite::new(descriptor.base(), vec![iteration; 64]),
            Some((
                indirect,
                DmaWrite::new(indirect_region.base(), vec![iteration; 8]),
            )),
        )));
        assert!(poll_once_pending(&mut submission_future).await);
        drop(submission_future);

        // Then: both cancelled futures preserve every commit surface exactly.
        assert_eq!(memory_image(&memory)?, before_memory);
        assert_eq!(state.dma_space.generation(), before_mapping_generation);
        assert_eq!(state.generation, before_runner_generation);
        assert_eq!(state.wake_generation, before_wake_generation);
        assert_eq!(queue_latch_values(&state), before_latches);
        assert_eq!(state.backend.trace_events(), before_trace);
        assert_eq!(
            *observer
                .observations
                .lock()
                .expect("observation probe lock available"),
            before_observations
        );
        drop(state);
    }
    Ok(())
}

async fn poll_once_pending<F: Future>(future: &mut std::pin::Pin<Box<F>>) -> bool {
    poll_fn(|context| std::task::Poll::Ready(future.as_mut().poll(context).is_pending())).await
}

fn queue_latch_values(state: &crate::runner::state::DeviceCore) -> Vec<Vec<u8>> {
    queue_latch_offsets()
        .into_iter()
        .map(|(offset, _)| {
            let register = pdma_model_contract::abi::mmio::decode_read(offset, 8)
                .or_else(|_| pdma_model_contract::abi::mmio::decode_read(offset, 4))
                .expect("known queue latch decodes");
            crate::runner::register_endpoint::prepare_read(state, register, offset).response
        })
        .collect()
}

type MemoryImage = (
    u64,
    Vec<(hardware_interface_runtime::MemoryRegion, Vec<u8>)>,
);

fn memory_image(memory: &hardware_interface_runtime::SharedMemory) -> TestResult<MemoryImage> {
    use hardware_interface_runtime::MemoryAccess;

    let regions = memory.regions();
    let mut image = Vec::with_capacity(regions.len());
    for region in regions {
        let length = usize::try_from(region.length())?;
        let mut bytes = vec![0; length];
        memory.read(region.base(), region.alignment(), &mut bytes)?;
        image.push((region, bytes));
    }
    Ok((memory.revision().raw(), image))
}
