//! Atomic region installation through the public memory API.

use std::error::Error;
use std::sync::{Arc, Barrier};
use std::thread;

use hardware_interface_runtime::{
    LocalMemoryStorage, Memory, MemoryAccess, MemoryAddress, MemoryAlignment, MemoryError,
    MemoryRegion, SharedMemory,
};

type TestResult = Result<(), Box<dyn Error>>;

fn region(base: u64, length: u64) -> Result<MemoryRegion, Box<dyn Error>> {
    Ok(MemoryRegion::try_new(
        MemoryAddress::new(base),
        length,
        MemoryAlignment::try_new(1)?,
    )?)
}

#[test]
fn failed_batch_preserves_existing_regions_and_bytes() -> TestResult {
    // Given installed sentinel bytes and a valid addition preceding an overlap.
    let memory = SharedMemory::new();
    let installed = region(0x1000, 64)?;
    let addition = region(0x2000, 64)?;
    memory.add_regions(&[installed])?;
    let alignment = MemoryAlignment::try_new(1)?;
    memory.write(installed.base(), alignment, &[0xa5; 4])?;

    // When the complete batch is rejected.
    let result = memory.add_regions(&[addition, installed]);

    // Then no prefix is installed and existing bytes survive.
    assert_eq!(
        result,
        Err(MemoryError::Overlap {
            first: installed,
            second: installed
        })
    );
    assert_eq!(memory.regions()?, vec![installed]);
    let mut bytes = [0; 4];
    memory.read(installed.base(), alignment, &mut bytes)?;
    assert_eq!(bytes, [0xa5; 4]);
    Ok(())
}

#[test]
fn repeated_addition_rejects_duplicate_without_consuming_capacity() -> TestResult {
    // Given local memory with one installed region and one remaining slot.
    let memory = Memory::<LocalMemoryStorage>::with_region_capacity(2);
    let first = region(0x1000, 64)?;
    let second = region(0x2000, 64)?;
    memory.add_regions(&[first])?;

    // When another operation attempts to install the same region.
    let result = memory.add_regions(&[first]);

    // Then overlap is rejected and the remaining slot stays available.
    assert_eq!(
        result,
        Err(MemoryError::Overlap {
            first,
            second: first
        })
    );
    memory.add_regions(&[second])?;
    assert_eq!(memory.regions()?, vec![first, second]);
    Ok(())
}

#[test]
fn independent_stores_validate_their_own_regions() -> TestResult {
    // Given independent stores with equal region counts but different addresses.
    let first = SharedMemory::new();
    let second = SharedMemory::new();
    let a = region(0x1000, 64)?;
    let b = region(0x2000, 64)?;
    first.add_regions(&[a])?;
    second.add_regions(&[b])?;
    first.write(a.base(), MemoryAlignment::ONE, &[0xaa])?;
    second.write(b.base(), MemoryAlignment::ONE, &[0x55])?;

    // When identical metadata is added to each store independently.
    let first_result = first.add_regions(&[b]);
    let second_result = second.add_regions(&[b]);

    // Then the destination's contents alone decide overlap.
    assert_eq!(first_result, Ok(()));
    assert_eq!(
        second_result,
        Err(MemoryError::Overlap {
            first: b,
            second: b
        })
    );
    assert_eq!(first.regions()?, vec![a, b]);
    assert_eq!(second.regions()?, vec![b]);
    let mut first_bytes = [0];
    let mut second_bytes = [0];
    first.read(a.base(), MemoryAlignment::ONE, &mut first_bytes)?;
    second.read(b.base(), MemoryAlignment::ONE, &mut second_bytes)?;
    assert_eq!(first_bytes, [0xaa]);
    assert_eq!(second_bytes, [0x55]);
    Ok(())
}

#[test]
fn competing_overlapping_additions_have_one_winner() -> TestResult {
    // Given two writers synchronized at the public add operation.
    let memory = SharedMemory::new();
    let shared_region = region(0x1000, 64)?;
    let start = Arc::new(Barrier::new(2));
    let writers = [(), ()].map(|()| {
        let memory = memory.clone();
        let start = Arc::clone(&start);
        thread::spawn(move || {
            start.wait();
            memory.add_regions(&[shared_region])
        })
    });

    // When both compete to add the same address range.
    let results = writers
        .into_iter()
        .map(|writer| writer.join().map_err(|_| "memory writer panicked"))
        .collect::<Result<Vec<_>, _>>()?;

    // Then one succeeds and one observes the installed overlap.
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| **result
                == Err(MemoryError::Overlap {
                    first: shared_region,
                    second: shared_region,
                }))
            .count(),
        1
    );
    assert_eq!(memory.regions()?, vec![shared_region]);
    Ok(())
}

#[test]
fn capacity_rejection_preserves_regions_and_bytes() -> TestResult {
    // Given one occupied slot and only one free slot for a two-region batch.
    let memory = SharedMemory::with_region_capacity(2);
    let installed = region(0x1000, 64)?;
    memory.add_regions(&[installed])?;
    memory.write(installed.base(), MemoryAlignment::ONE, &[0x5a])?;

    // When adding the batch exceeds the final table capacity.
    let result = memory.add_regions(&[region(0x2000, 64)?, region(0x3000, 64)?]);

    // Then no additions or changes to the sentinel are visible.
    assert_eq!(
        result,
        Err(MemoryError::RegionTableExhausted { entries: 3 })
    );
    assert_eq!(memory.regions()?, vec![installed]);
    let mut bytes = [0];
    memory.read(installed.base(), MemoryAlignment::ONE, &mut bytes)?;
    assert_eq!(bytes, [0x5a]);
    Ok(())
}

#[test]
fn backing_exhaustion_discards_allocated_batch_prefix() -> TestResult {
    // Given a sentinel and a batch whose second backing exceeds Vec's size limit.
    let memory = SharedMemory::new();
    let installed = region(0, 64)?;
    memory.add_regions(&[installed])?;
    memory.write(installed.base(), MemoryAlignment::ONE, &[0x5a])?;
    let impossible = region(0x2000, u64::try_from(isize::MAX)? + 1)?;

    // When the valid first addition allocates but the second allocation fails.
    let result = memory.add_regions(&[region(0x1000, 64)?, impossible]);

    // Then installed metadata and sentinel bytes remain intact.
    assert_eq!(
        result,
        Err(MemoryError::StorageExhausted {
            length: impossible.length()
        })
    );
    assert_eq!(memory.regions()?, vec![installed]);
    let mut bytes = [0];
    memory.read(installed.base(), MemoryAlignment::ONE, &mut bytes)?;
    assert_eq!(bytes, [0x5a]);
    Ok(())
}

#[test]
fn batch_overlap_is_reported_in_declaration_order() -> TestResult {
    // Given overlapping metadata with descending base addresses.
    let memory = SharedMemory::new();
    let first = region(0x2020, 64)?;
    let second = region(0x2000, 64)?;

    // When the batch is added to empty storage.
    let result = memory.add_regions(&[first, second]);

    // Then the declared order is retained in the error and nothing installs.
    assert_eq!(result, Err(MemoryError::Overlap { first, second }));
    assert!(memory.regions()?.is_empty());
    Ok(())
}

#[test]
fn empty_addition_is_a_noop_at_capacity() -> TestResult {
    // Given full memory containing a sentinel.
    let memory = SharedMemory::with_region_capacity(1);
    let installed = region(0x1000, 64)?;
    memory.add_regions(&[installed])?;
    memory.write(installed.base(), MemoryAlignment::ONE, &[0xa5])?;

    // When an empty batch is submitted.
    memory.add_regions(&[])?;

    // Then no capacity is needed and bytes remain intact.
    assert_eq!(memory.regions()?, vec![installed]);
    let mut bytes = [0];
    memory.read(installed.base(), MemoryAlignment::ONE, &mut bytes)?;
    assert_eq!(bytes, [0xa5]);
    Ok(())
}

#[test]
fn successful_batch_sorts_regions_and_preserves_existing_bytes() -> TestResult {
    // Given an installed sentinel between the addresses of two new regions.
    let memory = Memory::<LocalMemoryStorage>::new();
    let installed = region(0x2000, 4)?;
    let first = region(0x1000, 4)?;
    let last = region(0x3000, 4)?;
    memory.add_regions(&[installed])?;
    memory.write(installed.base(), MemoryAlignment::ONE, &[0xa5; 4])?;

    // When a batch arrives in descending address order.
    memory.add_regions(&[last, first])?;

    // Then metadata is sorted, existing bytes survive, and new backing is zeroed.
    assert_eq!(memory.regions()?, vec![first, installed, last]);
    for (region, expected) in [(first, [0; 4]), (installed, [0xa5; 4]), (last, [0; 4])] {
        let mut bytes = [0xff; 4];
        memory.read(region.base(), MemoryAlignment::ONE, &mut bytes)?;
        assert_eq!(bytes, expected);
    }
    Ok(())
}

#[test]
fn competing_disjoint_additions_both_succeed() -> TestResult {
    // Given clone-shared memory and disjoint address ranges.
    let memory = SharedMemory::new();
    let first = region(0x1000, 64)?;
    let second = region(0x2000, 64)?;
    let start = Arc::new(Barrier::new(2));

    // When the writers begin their additions concurrently.
    let writers = [first, second].map(|addition| {
        let memory = memory.clone();
        let start = Arc::clone(&start);
        thread::spawn(move || {
            start.wait();
            memory.add_regions(&[addition])
        })
    });

    // Then both operations succeed and both regions are installed.
    for writer in writers {
        writer.join().map_err(|_| "memory writer panicked")??;
    }
    assert_eq!(memory.regions()?, vec![first, second]);
    Ok(())
}
