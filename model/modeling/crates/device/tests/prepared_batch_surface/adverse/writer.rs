use super::*;

pub(super) async fn run() -> support::TestResult<bool> {
    // Given: one writer holds the real memory transaction ahead of a bootstrap batch.
    let (handle, _, memory) = support::observed_handle_with_memory();
    let stage = handle.stage_memory_regions(&[128, 64, 128]).await?;
    let activation = support::activation(&stage, DeviceEpoch::initial())?;
    let region = MemoryRegion::try_new(
        MemoryAddress::new(0x5_0000),
        64,
        MemoryAlignment::try_new(64)?,
    )?;
    let writer_memory = std::sync::Arc::clone(&memory);
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let (batch_started_tx, batch_started_rx) = std::sync::mpsc::channel();
    let (batch_done_tx, batch_done_rx) = std::sync::mpsc::channel();

    // When: the batch starts while the competing writer owns the transaction view.
    let writer = std::thread::spawn(move || {
        writer_memory.transaction(|transaction| {
            entered_tx.send(())?;
            release_rx.recv()?;
            let prepared = transaction.prepare_regions(&[region])?;
            transaction.install_prepared(prepared);
            Ok::<_, Box<dyn std::error::Error + Send + Sync>>(())
        })
    });
    entered_rx.recv()?;
    let batch_handle = handle.clone();
    let batch = std::thread::spawn(move || -> support::TestResult {
        batch_started_tx.send(())?;
        tokio::runtime::Builder::new_current_thread()
            .build()?
            .block_on(
                batch_handle.bootstrap_queue_batch(QueueBootstrapBatch::new(stage, activation)),
            )?;
        batch_done_tx.send(())?;
        Ok(())
    });
    batch_started_rx.recv()?;
    let blocked_before_release = matches!(
        batch_done_rx.try_recv(),
        Err(std::sync::mpsc::TryRecvError::Empty)
    );
    assert!(blocked_before_release);
    release_tx.send(())?;
    writer.join().map_err(|_| "writer thread panicked")??;
    batch.join().map_err(|_| "batch thread panicked")??;
    batch_done_rx.recv()?;

    // Then: both commits survive in serial revision order with no stale inner install.
    let serialized = memory.revision().raw() == 2
        && memory.regions().len() == 4
        && memory.regions().contains(&region);
    assert!(serialized);
    Ok(serialized)
}
