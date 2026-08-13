#![allow(missing_docs, reason = "integration test crate")]
#![allow(clippy::expect_used, reason = "checked test fixture operations")]

mod support;

use std::sync::{Arc, Barrier};
use std::thread;

use hardware_interface_runtime::{
    MemoryAccess, MemoryAddress, MemoryAlignment, MemoryError, SharedMemory,
};
use support::region;

#[test]
fn memory_transaction_install_is_infallible_while_competing_writer_waits() {
    // Given a clone-shared store and a writer released while its transaction is held.
    let memory = SharedMemory::new();
    let competitor = memory.clone();
    let start = Arc::new(Barrier::new(2));
    let started = Arc::clone(&start);
    let writer = thread::spawn(move || {
        started.wait();
        competitor
            .prepare_regions(&[region(0x2000, 64, 64)])
            .expect("competitor prepares after lock release")
            .try_install(&competitor)
    });

    // When the transaction validates, releases the competitor, and installs locally.
    let installed = memory.transaction(|transaction| {
        let prepared = transaction
            .prepare_regions(&[region(0x1000, 64, 64)])
            .expect("transaction-local validation");
        start.wait();
        transaction.install_prepared(prepared)
    });
    let competitor_result = writer.join().expect("competitor joins");

    // Then the local install cannot become stale and the later writer advances revision.
    assert_eq!(installed.raw(), 1);
    assert_eq!(competitor_result, Ok(()));
    assert_eq!(memory.revision().raw(), 2);
}

#[test]
fn memory_transaction_unwind_before_install_leaves_store_usable() {
    // Given a store whose exclusive transaction will unwind after validation.
    let memory = SharedMemory::new();

    // When the closure panics before consuming its prepared token.
    let unwind = std::panic::catch_unwind(|| {
        memory.transaction(|transaction| {
            let _prepared = transaction
                .prepare_regions(&[region(0x1000, 64, 64)])
                .expect("transaction-local validation");
            panic!("test unwind before install");
        });
    });

    // Then no region committed and poison recovery preserves a usable store.
    assert!(unwind.is_err());
    assert_eq!(memory.revision().raw(), 0);
    assert_eq!(memory.regions(), Vec::new());
}

#[test]
fn transaction_draft_checks_regions_and_writes_before_infallible_install() {
    // Given: one committed region and a transaction-local prospective region.
    let memory = SharedMemory::new();
    memory
        .prepare_regions(&[region(0x1000, 64, 64)])
        .expect("initial region prepares")
        .try_install(&memory)
        .expect("initial region installs");

    // When: a draft prepares its region and both existing/new-region writes before install.
    let revision = memory
        .transaction(|transaction| {
            let mut draft = transaction.try_draft()?;
            draft.prepare_regions(&[region(0x2000, 64, 64)])?;
            draft.write(MemoryAddress::new(0x1000), MemoryAlignment::ONE, &[1, 2])?;
            draft.write(MemoryAddress::new(0x2000), MemoryAlignment::ONE, &[3, 4])?;
            Ok::<_, Box<dyn std::error::Error>>(transaction.install_draft(draft))
        })
        .expect("draft validates and installs");

    // Then: both writes appear atomically at the draft's single next revision.
    let mut existing = [0; 2];
    let mut added = [0; 2];
    memory
        .read(
            MemoryAddress::new(0x1000),
            MemoryAlignment::ONE,
            &mut existing,
        )
        .expect("existing bytes read");
    memory
        .read(MemoryAddress::new(0x2000), MemoryAlignment::ONE, &mut added)
        .expect("added bytes read");
    assert_eq!(revision.raw(), 2);
    assert_eq!(existing, [1, 2]);
    assert_eq!(added, [3, 4]);
}

#[test]
fn rejected_transaction_draft_preserves_regions_revision_and_bytes() {
    // Given: one committed region with sentinel bytes.
    let memory = SharedMemory::new();
    memory
        .prepare_regions(&[region(0x1000, 64, 64)])
        .expect("initial region prepares")
        .try_install(&memory)
        .expect("initial region installs");
    memory
        .write(MemoryAddress::new(0x1000), MemoryAlignment::ONE, &[0x5a])
        .expect("sentinel writes");
    let before_regions = memory.regions();
    let before_revision = memory.revision();

    // When: prospective overlap is rejected before its draft can install.
    let rejected = memory.transaction(|transaction| {
        let mut draft = transaction.try_draft().expect("draft clones");
        draft.prepare_regions(&[region(0x1000, 64, 64)])
    });

    // Then: the real store retains its exact region, revision, and sentinel byte.
    let mut sentinel = [0];
    memory
        .read(
            MemoryAddress::new(0x1000),
            MemoryAlignment::ONE,
            &mut sentinel,
        )
        .expect("sentinel reads");
    assert!(matches!(rejected, Err(MemoryError::Overlap { .. })));
    assert_eq!(memory.regions(), before_regions);
    assert_eq!(memory.revision(), before_revision);
    assert_eq!(sentinel, [0x5a]);
}

#[test]
fn transaction_draft_honors_region_capacity_without_partial_install() {
    // Given: a store limited below the proposed transaction size.
    let memory = SharedMemory::with_region_capacity(1);
    let regions = [region(0x1000, 64, 64), region(0x2000, 64, 64)];

    // When: the exclusive draft prepares both regions.
    let result = memory.transaction(|transaction| {
        let mut draft = transaction.try_draft()?;
        draft.prepare_regions(&regions)?;
        transaction.install_draft(draft);
        Ok::<_, MemoryError>(())
    });

    // Then: capacity rejection preserves revision and installed regions.
    assert!(matches!(
        result,
        Err(MemoryError::RegionTableExhausted { entries: 2 })
    ));
    assert_eq!(memory.revision().raw(), 0);
    assert!(memory.regions().is_empty());
}
