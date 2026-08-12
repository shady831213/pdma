#![allow(missing_docs, reason = "integration test crate")]
#![allow(clippy::expect_used, reason = "checked test fixture operations")]

mod support;

use hardware_interface_runtime::{
    MemoryAccess, MemoryAccessError, MemoryAddress, MemoryError, SharedMemory,
};
use support::{alignment, install, region};

#[test]
fn memory_prepare_rejects_storage_exhaustion_and_overlap_without_mutation() {
    // Given one installed region with sentinel bytes.
    let memory = SharedMemory::new();
    install(&memory, &[region(0x1000, 64, 64)]);
    memory
        .write(MemoryAddress::new(0x1000), alignment(1), &[0xaa])
        .expect("sentinel write");
    let before = memory.revision();

    // When an impossible allocation and an overlap are prepared.
    let exhaustion_store = SharedMemory::new();
    let exhausted = exhaustion_store.prepare_regions(&[region(0, u64::MAX, 1)]);
    let overlap = memory.prepare_regions(&[region(0x1020, 64, 32)]);

    // Then both fail without changing revision or bytes.
    assert_eq!(
        exhausted,
        Err(MemoryError::StorageExhausted { length: u64::MAX })
    );
    assert_eq!(
        overlap,
        Err(MemoryError::Overlap {
            first: region(0x1000, 64, 64),
            second: region(0x1020, 64, 32),
        })
    );
    assert_eq!(memory.revision(), before);
    let mut byte = [0];
    memory
        .read(MemoryAddress::new(0x1000), alignment(1), &mut byte)
        .expect("sentinel read");
    assert_eq!(byte, [0xaa]);
}

#[test]
fn memory_prepare_rejects_proposed_overlap_in_declaration_order() {
    // Given an empty store and two overlapping proposals in declaration order.
    let memory = SharedMemory::new();
    let first = region(0x2000, 64, 64);
    let second = region(0x2020, 64, 32);

    // When the proposed set is prepared.
    let result = memory.prepare_regions(&[first, second]);

    // Then the ordered pair is rejected without a serialization point.
    assert_eq!(result, Err(MemoryError::Overlap { first, second }));
    assert_eq!(memory.revision().raw(), 0);
    assert_eq!(memory.regions(), Vec::new());
}

#[test]
fn memory_prepared_drop_and_stale_install_do_not_mutate_store() {
    // Given two proposals from revision zero and one proposal to drop.
    let memory = SharedMemory::new();
    let dropped = memory
        .prepare_regions(&[region(0x1000, 64, 64)])
        .expect("droppable proposal");
    let stale = memory
        .prepare_regions(&[region(0x3000, 64, 64)])
        .expect("stale proposal");
    let winner = memory
        .prepare_regions(&[region(0x3000, 64, 64)])
        .expect("winning proposal");

    // When one drops, one installs, and the stale proposal is consumed.
    drop(dropped);
    winner.try_install(&memory).expect("winner installs");
    memory
        .write(MemoryAddress::new(0x3000), alignment(1), &[0xaa])
        .expect("winner sentinel");
    let stale_error = stale.try_install(&memory);

    // Then stale revision wins before mutation and only the winner exists.
    assert_eq!(
        stale_error,
        Err(MemoryError::StaleRevision {
            expected_revision: 0,
            actual_revision: 1,
        })
    );
    assert_eq!(memory.revision().raw(), 1);
    assert_eq!(
        memory.write(MemoryAddress::new(0x1000), alignment(1), &[1]),
        Err(MemoryAccessError::Unmapped {
            address: MemoryAddress::new(0x1000),
        })
    );
    assert_eq!(
        memory.write(MemoryAddress::new(0x2000), alignment(1), &[1]),
        Err(MemoryAccessError::Unmapped {
            address: MemoryAddress::new(0x2000),
        })
    );
    let mut winner_byte = [0];
    memory
        .read(MemoryAddress::new(0x3000), alignment(1), &mut winner_byte)
        .expect("winner remains readable");
    assert_eq!(winner_byte, [0xaa]);
}

#[test]
fn memory_empty_successful_install_advances_revision() {
    // Given an empty store and one prepared empty serialization point.
    let memory = SharedMemory::new();
    let prepared = memory.prepare_regions(&[]).expect("empty proposal");

    // When the empty proposal installs.
    let result = prepared.try_install(&memory);

    // Then success advances the monotonic revision exactly once.
    assert_eq!(result, Ok(()));
    assert_eq!(memory.revision().raw(), 1);
    assert_eq!(memory.regions(), Vec::new());
}
