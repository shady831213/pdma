use super::*;
use hardware_interface_runtime::{MemoryAccessError, MemoryError};

pub(super) mod writer;

pub(super) async fn run() -> support::TestResult {
    // Given: an active observed queue.
    let (active, observer) = support::observed_handle();
    let submission = support::bootstrap(&active).await?;
    let status = support::cursors(&active).await?;
    let before = active.snapshot().await;
    let observations = observer.snapshot().len();
    let endpoint = active.register_endpoint();

    // When: malformed individual register/memory accesses are attempted.
    assert!(endpoint.read(0, &mut [0; 1]).await.is_err());
    assert!(endpoint.write(0, &[0; 4]).await.is_err());
    assert!(
        endpoint
            .write(
                u32::try_from(GlobalRegisters::RESET.offset())?,
                &u32::MAX.to_le_bytes()
            )
            .await
            .is_err()
    );
    assert_eq!(
        active.write_memory(submission, &[0; 129]).await,
        Err(MemoryAccessError::CrossesRegion {
            address: MemoryAddress::new(submission.raw()),
            length: 129,
            region: MemoryRegion::try_new(
                MemoryAddress::new(submission.raw()),
                128,
                MemoryAlignment::try_new(64)?
            )?,
        })
    );

    // Then: each rejected access preserves its hardware data and emits no register observation.
    assert_eq!(active.snapshot().await, before);
    assert_eq!(observer.snapshot().len(), observations);
    assert_eq!(support::cursors(&active).await?, status);
    assert_eq!(active.read_memory(submission, 128).await?, vec![0; 128]);
    capacity_and_overlap()?;
    Ok(())
}

fn capacity_and_overlap() -> support::TestResult {
    // Given: bounded storage and an independently installed overlapping runtime region.
    let capacity_memory =
        std::sync::Arc::new(hardware_interface_runtime::SharedMemory::with_region_capacity(2));
    let (_, capacity_observer) = support::observed_handle_for_memory(&capacity_memory);
    let rings = [
        MemoryRegion::try_new(
            MemoryAddress::new(0x1_0000),
            128,
            MemoryAlignment::try_new(64)?,
        )?,
        MemoryRegion::try_new(
            MemoryAddress::new(0x1_0080),
            64,
            MemoryAlignment::try_new(64)?,
        )?,
        MemoryRegion::try_new(
            MemoryAddress::new(0x1_00c0),
            128,
            MemoryAlignment::try_new(64)?,
        )?,
    ];
    let overlap_memory = std::sync::Arc::new(hardware_interface_runtime::SharedMemory::new());
    let overlapping = MemoryRegion::try_new(
        MemoryAddress::new(0x1_0000),
        64,
        MemoryAlignment::try_new(64)?,
    )?;
    overlap_memory
        .prepare_regions(&[overlapping])?
        .try_install(&overlap_memory)?;
    let revision = overlap_memory.revision();
    let (_, overlap_observer) = support::observed_handle_for_memory(&overlap_memory);

    // When: runtime region preparation encounters storage capacity and overlap.
    assert_eq!(
        capacity_memory.prepare_regions(&rings),
        Err(MemoryError::RegionTableExhausted { entries: 3 })
    );
    assert_eq!(
        overlap_memory.prepare_regions(&rings),
        Err(MemoryError::Overlap {
            first: overlapping,
            second: rings[0]
        })
    );

    // Then: neither failed preparation changes storage or emits MMIO observations.
    assert!(capacity_memory.regions().is_empty());
    assert_eq!(capacity_memory.revision().raw(), 0);
    assert!(capacity_observer.snapshot().is_empty());
    assert_eq!(overlap_memory.revision(), revision);
    assert_eq!(overlap_memory.regions(), vec![overlapping]);
    assert!(overlap_observer.snapshot().is_empty());
    Ok(())
}
