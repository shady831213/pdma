use super::*;
use crate::{DmaWrite, SubmissionBatch};
use pdma_model_contract::semantic::RingCursor;
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
    let (device, _worker) = AsyncDevice::from_core_observed(
        crate::DeviceCore::new(
            Arc::clone(&memory),
            pdma_model_contract::semantic::ResourceConfig::default(),
        ),
        observer.clone(),
    );
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
        let before_memory = memory_image(&memory)?;
        let before_state = state_image(&handle).await;
        let before_observations = observer
            .observations
            .lock()
            .expect("observation probe lock available")
            .clone();
        let (release_sender, held) = hold_backend(&handle)?;

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
        release_sender.send(())?;
        held.join().map_err(|_| "holder thread panicked")?;

        // Then: both cancelled futures preserve every commit surface exactly.
        assert_eq!(memory_image(&memory)?, before_memory);
        assert_eq!(state_image(&handle).await, before_state);
        assert_eq!(
            *observer
                .observations
                .lock()
                .expect("observation probe lock available"),
            before_observations
        );
    }
    Ok(())
}

type StateImage = (
    u64,
    usize,
    usize,
    Vec<Vec<u8>>,
    Vec<pdma_model_contract::semantic::TraceEvent>,
);

async fn state_image(handle: &crate::runtime::AsyncDeviceHandle) -> StateImage {
    handle
        .shared
        .backend
        .interact(|state| {
            (
                state.core.dma.generation(),
                state.generation,
                state.wake_generation,
                queue_latch_values(&state.core),
                state.core.trace_events(),
            )
        })
        .await
}

fn hold_backend(
    handle: &crate::runtime::AsyncDeviceHandle,
) -> TestResult<(std::sync::mpsc::Sender<()>, std::thread::JoinHandle<()>)> {
    let (ready_sender, ready_receiver) = std::sync::mpsc::channel();
    let (release_sender, release_receiver) = std::sync::mpsc::channel();
    let shared = Arc::clone(&handle.shared);
    let held = std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("holder runtime")
            .block_on(shared.backend.hold_for_test(ready_sender, release_receiver));
    });
    ready_receiver.recv()?;
    Ok((release_sender, held))
}

async fn poll_once_pending<F: Future>(future: &mut std::pin::Pin<Box<F>>) -> bool {
    poll_fn(|context| std::task::Poll::Ready(future.as_mut().poll(context).is_pending())).await
}

fn queue_latch_values(state: &crate::DeviceCore) -> Vec<Vec<u8>> {
    queue_latch_offsets()
        .into_iter()
        .map(|(offset, _)| {
            let register = pdma_model_contract::abi::mmio::decode_read(offset, 8)
                .or_else(|_| pdma_model_contract::abi::mmio::decode_read(offset, 4))
                .expect("known queue latch decodes");
            crate::interface::mmio::prepare_read(state, register, offset).response
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
