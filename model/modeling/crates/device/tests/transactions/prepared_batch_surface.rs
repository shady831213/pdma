#![allow(missing_docs, reason = "integration test crate")]

use super::bus_batch::support;

use hardware_interface_runtime::{AccessOrigin, RegisterAccess};
use hardware_interface_runtime::{MemoryAddress, MemoryAlignment, MemoryRegion};
use pdma_model_contract::abi::DmaAddress;
use pdma_model_contract::semantic::DeviceEpoch;
use pdma_model_contract::semantic::RingCursor;
use pdma_model_device::{DmaWrite, QueueBootstrapBatch, SubmissionBatch};

mod adverse;
mod happy;

#[tokio::test]
async fn prepared_batch_adverse_surface() -> support::TestResult {
    adverse::run().await
}

#[tokio::test]
async fn prepared_batch_direct_order_surface() -> support::TestResult {
    // Given: a direct endpoint clone and a batch handle sharing one observation gate.
    let (handle, observer) = support::observed_handle();
    let endpoint = handle.register_endpoint();

    // When: a direct write reserves before a bootstrap batch on the same core.
    endpoint.write(0x100, &0_u64.to_le_bytes()).await?;
    let _ = support::bootstrap(&handle).await?;

    // Then: global sequences reflect commit order across both origins.
    let observations: [_; 2] = observer
        .snapshot()
        .try_into()
        .map_err(|_| "exactly two observations required")?;
    assert_eq!(observations[0].sequence, 0);
    assert_eq!(observations[0].origin, AccessOrigin::Direct);
    assert_eq!(observations[1].sequence, 1);
    assert_eq!(observations[1].origin, AccessOrigin::Batch);
    Ok(())
}

#[tokio::test]
async fn direct_and_batch_race_reserves_global_commit_order() -> support::TestResult {
    // Given: direct and batch operations released concurrently against one core gate.
    let (handle, observer) = support::observed_handle();
    let endpoint = handle.register_endpoint();
    let stage = handle.stage_memory_regions(&[128, 64, 128]).await?;
    let activation = support::activation(&stage, DeviceEpoch::initial())?;
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
    let direct_barrier = std::sync::Arc::clone(&barrier);
    let batch_barrier = std::sync::Arc::clone(&barrier);

    // When: both futures contend from the same release point.
    let direct = std::thread::spawn(move || -> support::TestResult {
        direct_barrier.wait();
        tokio::runtime::Builder::new_current_thread()
            .build()?
            .block_on(endpoint.write(0x100, &0_u64.to_le_bytes()))?;
        Ok(())
    });
    let batch_handle = handle.clone();
    let batch = std::thread::spawn(move || -> support::TestResult {
        batch_barrier.wait();
        tokio::runtime::Builder::new_current_thread()
            .build()?
            .block_on(
                batch_handle.bootstrap_queue_batch(QueueBootstrapBatch::new(stage, activation)),
            )?;
        Ok(())
    });
    barrier.wait();
    direct.join().map_err(|_| "direct race thread panicked")??;
    batch.join().map_err(|_| "batch race thread panicked")??;

    // Then: either winner is valid, but observations remain contiguous in commit order.
    let observations: [_; 2] = observer
        .snapshot()
        .try_into()
        .map_err(|_| "exactly two raced observations required")?;
    assert_eq!([observations[0].sequence, observations[1].sequence], [0, 1]);
    assert_ne!(observations[0].origin, observations[1].origin);
    Ok(())
}

#[tokio::test]
async fn memory_writer_serializes_before_or_after_batch() -> support::TestResult {
    // Given: a batch and a generic transaction writer sharing the same memory store.
    let (handle, _, memory) = support::observed_handle_with_memory();
    let stage = handle.stage_memory_regions(&[128, 64, 128]).await?;
    let activation = support::activation(&stage, DeviceEpoch::initial())?;
    let alignment = MemoryAlignment::try_new(64)?;
    let writer_region = MemoryRegion::try_new(MemoryAddress::new(0x5_0000), 64, alignment)?;
    let writer_memory = std::sync::Arc::clone(&memory);
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let (batch_started_tx, batch_started_rx) = std::sync::mpsc::channel();
    let (batch_done_tx, batch_done_rx) = std::sync::mpsc::channel();

    // When: the generic writer transaction races the batch transaction.
    let writer = std::thread::spawn(move || {
        writer_memory.transaction(|transaction| {
            entered_tx.send(())?;
            release_rx.recv()?;
            let prepared = transaction.prepare_regions(&[writer_region])?;
            transaction.install_prepared(prepared);
            Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
        })
    });
    entered_rx.recv()?;
    let batch_handle = handle.clone();
    let batch = std::thread::spawn(move || -> support::TestResult {
        tokio::runtime::Builder::new_current_thread()
            .build()?
            .block_on(async {
                batch_started_tx.send(())?;
                Ok::<_, Box<dyn std::error::Error + Send + Sync>>(
                    batch_handle
                        .bootstrap_queue_batch(QueueBootstrapBatch::new(stage, activation))
                        .await,
                )
            })??;
        batch_done_tx.send(())?;
        Ok(())
    });
    batch_started_rx.recv()?;
    assert!(matches!(
        batch_done_rx.try_recv(),
        Err(std::sync::mpsc::TryRecvError::Empty)
    ));
    release_tx.send(())?;
    writer
        .join()
        .map_err(|_| "memory writer thread panicked")??;
    batch.join().map_err(|_| "batch race thread panicked")??;
    batch_done_rx.recv()?;

    // Then: both commits survive and the store advances exactly twice without a stale install.
    assert_eq!(memory.revision().raw(), 2);
    assert_eq!(memory.regions().len(), 4);
    assert!(memory.regions().contains(&writer_region));
    Ok(())
}
