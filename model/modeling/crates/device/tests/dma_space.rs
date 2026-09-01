//! Checked DMA allocation, atomic staging, access, and backend-persistence tests.

use pdma_model_contract::abi::DmaAddress;
use pdma_model_contract::semantic::ids::BufferId;
use pdma_model_device::{MappedDmaRegion, PdmaMemoryError, PdmaMemoryMappings};

#[test]
fn staged_batch_returns_regions_and_commits_all_or_none() {
    // Given an empty checked DMA space.
    let mut space = PdmaMemoryMappings::new(std::sync::Arc::new(
        hardware_interface_runtime::SharedMemory::new(),
    ));

    // When three differently sized regions are staged and committed as one batch.
    let stage = space.stage(&[3, 0, 65]).expect("valid stage");
    let proposed = stage.regions().to_vec();
    let committed = space.commit(stage).expect("atomic commit");

    // Then deterministic IDs and aligned DMA addresses become visible together.
    assert_eq!(committed, proposed);
    assert_eq!(space.validate_region_set(&committed), Ok(()));
    assert_eq!(space.generation(), 1);
    let &[first, empty, third] = committed.as_slice() else {
        panic!("three regions must commit");
    };
    assert_eq!(first.object(), BufferId::new(0));
    assert_eq!(first.base(), DmaAddress::new(0x1_0000));
    assert_eq!(first.length(), 3);
    assert_eq!(empty.object(), BufferId::new(1));
    assert_eq!(empty.base(), DmaAddress::new(0x1_0040));
    assert_eq!(empty.length(), 0);
    assert_eq!(third.object(), BufferId::new(2));
    assert_eq!(third.base(), DmaAddress::new(0x1_0080));
    assert_eq!(third.length(), 65);
    assert_eq!(space.region_for_object(BufferId::new(2)), Some(third));
    assert_eq!(
        space.region_containing(DmaAddress::new(0x1_0081)),
        Some(third)
    );
}

#[test]
fn committed_region_exposes_object_base_and_length() {
    // Given: a fresh checked DMA space.
    let mut space = PdmaMemoryMappings::new(std::sync::Arc::new(
        hardware_interface_runtime::SharedMemory::new(),
    ));

    // When: one semantic-sized allocation commits.
    let committed = space.allocate(7).expect("allocation");

    // Then: its object, aligned base, length, and zeroed bytes are one exact mapping.
    assert_eq!(committed.object(), BufferId::new(0));
    assert_eq!(committed.base(), DmaAddress::new(0x1_0000));
    assert_eq!(committed.length(), 7);
    assert_eq!(space.region_for_object(committed.object()), Some(committed));
    assert_eq!(space.read_memory(committed.base(), 7), Ok(vec![0; 7]));
    assert_eq!(space.generation(), 1);
}

#[test]
fn allocation_exhaustion_preserves_allocator_state() {
    // Given: one committed mapping with sentinel bytes and its next allocation staged.
    let mut space = PdmaMemoryMappings::new(std::sync::Arc::new(
        hardware_interface_runtime::SharedMemory::new(),
    ));
    let first = space.allocate(4).expect("first allocation");
    space
        .write_memory(first.base(), &[1, 2, 3, 4])
        .expect("sentinel write");
    let generation_before = space.generation();

    // When: an allocation cannot fit in the bounded u64 DMA address space.
    let exhausted = space.allocate(u64::MAX);

    // Then: ID/base/mapping/bytes remain unchanged and the next success advances once.
    assert_eq!(exhausted, Err(PdmaMemoryError::AddressExhausted));
    assert_eq!(space.generation(), generation_before);
    assert_eq!(space.read_memory(first.base(), 4), Ok(vec![1, 2, 3, 4]));
    assert_eq!(space.region_for_object(BufferId::new(1)), None);
    let next = space.allocate(4).expect("next allocation");
    assert_eq!(next.object(), BufferId::new(1));
    assert_eq!(next.base(), DmaAddress::new(0x1_0040));
    assert_eq!(next.length(), 4);
    assert_eq!(space.read_memory(next.base(), 4), Ok(vec![0; 4]));
    assert_eq!(space.generation(), generation_before + 1);
    eprintln!(
        "exhausted={exhausted:?} next_object={} next_base={:#x} next_length={} \
         first_bytes={:?} next_bytes={:?} generation={}",
        next.object().raw(),
        next.base().raw(),
        next.length(),
        space.read_memory(first.base(), 4).expect("sentinel bytes"),
        space.read_memory(next.base(), 4).expect("next bytes"),
        space.generation(),
    );
}

#[test]
fn dropped_and_conflicting_stages_preserve_allocator_mapping_and_bytes() {
    // Given one mapped region with nonzero data and two stages from the same generation.
    let mut space = PdmaMemoryMappings::new(std::sync::Arc::new(
        hardware_interface_runtime::SharedMemory::new(),
    ));
    let initial = space.allocate(4).expect("initial allocation");
    space
        .write_memory(initial.base(), &[1, 2, 3, 4])
        .expect("write");
    let dropped = space.stage(&[8]).expect("droppable stage");
    let winner = space.stage(&[8]).expect("winning stage");
    let conflicting = space.stage(&[8]).expect("conflicting stage");

    // When one stage is dropped, one commits, and the conflicting stage is retried.
    drop(dropped);
    let committed = space.commit(winner).expect("winning commit");
    let stale = space.commit(conflicting);

    // Then only the winning proposal changes allocation state and no bytes are altered.
    assert_eq!(
        stale,
        Err(PdmaMemoryError::StaleStage {
            expected_generation: 1,
            actual_generation: 2,
        })
    );
    assert_eq!(space.generation(), 2);
    assert_eq!(space.read_memory(initial.base(), 4), Ok(vec![1, 2, 3, 4]));
    assert_eq!(
        space.region_for_object(BufferId::new(1)),
        committed.first().copied()
    );
    assert_eq!(space.region_for_object(BufferId::new(2)), None);
}

#[test]
fn rejects_exhausted_overlapping_unmapped_and_cross_region_access() {
    // Given two adjacent allocations and a conflicting stale proposal.
    let mut space = PdmaMemoryMappings::new(std::sync::Arc::new(
        hardware_interface_runtime::SharedMemory::new(),
    ));
    let first = space.allocate(64).expect("first allocation");
    let stale_proposal = space.stage(&[64]).expect("stale proposal");
    let second = space.allocate(64).expect("second allocation");
    space
        .write_memory(first.base(), &[0xaa])
        .expect("sentinel write");
    let malformed_overlap = MappedDmaRegion::try_new(
        second.object(),
        first.base().checked_add(32).expect("overlap base"),
        second.length(),
    )
    .expect("checked malformed region");
    let misaligned = MappedDmaRegion::try_new(
        second.object(),
        second.base().checked_add(1).expect("misaligned base"),
        second.length(),
    )
    .expect("checked misaligned region");
    let mismatched = MappedDmaRegion::try_new(
        second.object(),
        second.base().checked_add(64).expect("mismatched base"),
        second.length(),
    )
    .expect("checked mismatched region");

    // When malformed extents and accesses are attempted.
    let exhausted = space.stage(&[u64::MAX]);
    let overlapping = space.validate_region_set(&[first, malformed_overlap]);
    let invalid_alignment = space.validate_region_set(&[misaligned]);
    let invalid_mapping = space.validate_region_set(&[mismatched]);
    let stale_conflict = space.commit(stale_proposal);
    let unmapped = space.read_memory(DmaAddress::new(0), 1);
    let crossing = space.write_memory(
        first.base().checked_add(63).expect("last byte"),
        &[0xbb, 0xcc],
    );

    // Then each request is rejected without changing mappings or bytes.
    assert_eq!(exhausted, Err(PdmaMemoryError::AddressExhausted));
    assert_eq!(overlapping, Err(PdmaMemoryError::OverlappingRegion));
    assert_eq!(invalid_alignment, Err(PdmaMemoryError::InvalidRegion));
    assert_eq!(invalid_mapping, Err(PdmaMemoryError::InvalidRegion));
    assert_eq!(
        stale_conflict,
        Err(PdmaMemoryError::StaleStage {
            expected_generation: 1,
            actual_generation: 2,
        })
    );
    assert_eq!(unmapped, Err(PdmaMemoryError::Unmapped));
    assert_eq!(crossing, Err(PdmaMemoryError::CrossesRegion));
    assert_eq!(space.read_memory(first.base(), 1), Ok(vec![0xaa]));
    assert_eq!(space.read_memory(second.base(), 1), Ok(vec![0]));
    eprintln!(
        "generation={} first={:?} second={:?} exhausted={exhausted:?} overlap={overlapping:?} stale={stale_conflict:?} unmapped={unmapped:?} crossing={crossing:?}",
        space.generation(),
        space.read_memory(first.base(), 1).expect("first sentinel"),
        space
            .read_memory(second.base(), 1)
            .expect("second sentinel")
    );
}

#[test]
fn zero_length_region_accepts_only_empty_dma_address_access_at_base() {
    // Given a committed zero-length region followed by a non-empty mapping.
    let mut space = PdmaMemoryMappings::new(std::sync::Arc::new(
        hardware_interface_runtime::SharedMemory::new(),
    ));
    let empty = space.allocate(0).expect("zero-length allocation");
    let following = space.allocate(1).expect("following allocation");

    // When empty and non-empty accesses begin at the zero-length region's base.
    let empty_read = space.read_memory(empty.base(), 0);
    let empty_write = space.write_memory(empty.base(), &[]);
    let non_empty_read = space.read_memory(empty.base(), 1);

    // Then only empty access at the exact base succeeds without touching the next mapping.
    assert_eq!(empty_read, Ok(Vec::new()));
    assert_eq!(empty_write, Ok(()));
    assert_eq!(non_empty_read, Err(PdmaMemoryError::CrossesRegion));
    assert_eq!(space.read_memory(following.base(), 1), Ok(vec![0]));
    assert_eq!(
        space.read_memory(empty.base().checked_add(1).expect("reserved byte"), 0),
        Err(PdmaMemoryError::Unmapped)
    );
}
