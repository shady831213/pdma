use super::*;

pub(in crate::transactions) fn run() -> support::TestResult<bool> {
    // Given: one writer holds the real memory transaction ahead of runtime allocation.
    let (handle, _, memory) = support::observed_handle_with_memory();
    let region = MemoryRegion::try_new(
        MemoryAddress::new(0x5_0000),
        64,
        MemoryAlignment::try_new(64)?,
    )?;
    let writer_memory = std::sync::Arc::clone(&memory);
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let (mapping_started_tx, mapping_started_rx) = std::sync::mpsc::channel();
    let (mapping_done_tx, mapping_done_rx) = std::sync::mpsc::channel();

    // When: the mapping starts while the competing writer owns the transaction view.
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
    let mapping = std::thread::spawn(move || -> support::TestResult {
        mapping_started_tx.send(())?;
        handle.allocate(128)?;
        mapping_done_tx.send(())?;
        Ok(())
    });
    mapping_started_rx.recv()?;
    let blocked_before_release = matches!(
        mapping_done_rx.try_recv(),
        Err(std::sync::mpsc::TryRecvError::Empty)
    );
    assert!(blocked_before_release);
    release_tx.send(())?;
    writer.join().map_err(|_| "writer thread panicked")??;
    mapping.join().map_err(|_| "mapping thread panicked")??;
    mapping_done_rx.recv()?;

    // Then: both commits survive in serial revision order with no stale inner install.
    let allocated = MemoryRegion::try_new(
        MemoryAddress::new(0x1_0000),
        128,
        MemoryAlignment::try_new(64)?,
    )?;
    let serialized = memory.revision().raw() == 2 && memory.regions() == vec![allocated, region];
    assert!(serialized);
    Ok(serialized)
}
