use std::error::Error;
use std::sync::Arc;

use hardware_interface_runtime::{MemoryRegion, SharedMemory};
use pdma_model_contract::{BufferId, DmaAddress, PdmaMemoryRegion};
use pdma_model_device::{PdmaMemoryError, PdmaMemoryMappings};

use super::{TestResult, require_error};

#[derive(Debug, PartialEq, Eq)]
struct MemoryState {
    mappings: Vec<PdmaMemoryRegion>,
    bytes: Vec<Vec<u8>>,
    generation: u64,
    runtime_regions: Vec<MemoryRegion>,
}

#[test]
fn memory_fate_rows_have_literal_leaf_sources() -> TestResult {
    address_exhaustion()?;
    storage_exhaustion()?;
    proposal_failures()?;
    access_failures()
}

fn address_exhaustion() -> TestResult {
    let (space, memory) = mappings(SharedMemory::new());
    let before = snapshot(&space, &memory)?;
    let error = require_error(space.stage(&[u64::MAX]))?;
    assert_eq!(error, PdmaMemoryError::AddressExhausted);
    assert!(error.source().is_none());
    assert_unchanged(&space, &memory, &before)
}

fn storage_exhaustion() -> TestResult {
    let (mut space, memory) = mappings(SharedMemory::with_region_capacity(0));
    let before = snapshot(&space, &memory)?;
    let error = require_error(space.allocate(1))?;
    assert_eq!(error, PdmaMemoryError::StorageExhausted);
    assert!(error.source().is_none());
    assert_unchanged(&space, &memory, &before)
}

fn proposal_failures() -> TestResult {
    let (mut space, memory) = mappings(SharedMemory::new());
    let first = space.allocate(64)?;
    let stale = space.stage(&[64])?;
    let second = space.allocate(64)?;
    let overlap = PdmaMemoryRegion::try_new(second.object(), first.base(), second.length())?;
    let before_overlap = snapshot(&space, &memory)?;
    let overlapping = require_error(space.validate_region_set(&[first, overlap]))?;
    assert_eq!(overlapping, PdmaMemoryError::OverlappingRegion);
    assert!(overlapping.source().is_none());
    assert_unchanged(&space, &memory, &before_overlap)?;
    let misaligned = PdmaMemoryRegion::try_new(
        second.object(),
        second.base().checked_add(1)?,
        second.length(),
    )?;
    let before_invalid = snapshot(&space, &memory)?;
    let invalid = require_error(space.validate_region_set(&[misaligned]))?;
    assert_eq!(invalid, PdmaMemoryError::InvalidRegion);
    assert!(invalid.source().is_none());
    assert_unchanged(&space, &memory, &before_invalid)?;
    let before_stale = snapshot(&space, &memory)?;
    let stale_error = require_error(space.commit(stale))?;
    assert_eq!(
        stale_error,
        PdmaMemoryError::StaleStage {
            expected_generation: 1,
            actual_generation: 2,
        }
    );
    assert!(stale_error.source().is_none());
    assert_unchanged(&space, &memory, &before_stale)
}

fn access_failures() -> TestResult {
    let (mut space, memory) = mappings(SharedMemory::new());
    let first = space.allocate(64)?;
    space.write_memory(first.base(), &[0x5a])?;
    let before_unmapped = snapshot(&space, &memory)?;
    let unmapped = require_error(space.read_memory(DmaAddress::new(0), 1))?;
    assert_eq!(unmapped, PdmaMemoryError::Unmapped);
    assert!(unmapped.source().is_none());
    assert_unchanged(&space, &memory, &before_unmapped)?;
    let before_crosses = snapshot(&space, &memory)?;
    let crosses = require_error(space.write_memory(first.base().checked_add(63)?, &[1, 2]))?;
    assert_eq!(crosses, PdmaMemoryError::CrossesRegion);
    assert!(crosses.source().is_none());
    assert_unchanged(&space, &memory, &before_crosses)
}

fn mappings(memory: SharedMemory) -> (PdmaMemoryMappings, Arc<SharedMemory>) {
    let memory = Arc::new(memory);
    (PdmaMemoryMappings::new(Arc::clone(&memory)), memory)
}

fn snapshot(space: &PdmaMemoryMappings, memory: &SharedMemory) -> TestResult<MemoryState> {
    let mappings = (0..space.mapped_region_count())
        .map(|raw| {
            space
                .region_for_object(BufferId::new(raw))
                .ok_or("mapping absent")
        })
        .collect::<Result<Vec<_>, _>>()?;
    let bytes = mappings
        .iter()
        .map(|region| space.read_memory(region.base(), region.length()))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(MemoryState {
        mappings,
        bytes,
        generation: space.generation(),
        runtime_regions: memory.regions(),
    })
}

fn assert_unchanged(
    space: &PdmaMemoryMappings,
    memory: &SharedMemory,
    before: &MemoryState,
) -> TestResult {
    let after = snapshot(space, memory)?;
    assert_eq!(after.mappings, before.mappings);
    assert_eq!(after.bytes, before.bytes);
    assert_eq!(after.generation, before.generation);
    assert_eq!(after.runtime_regions, before.runtime_regions);
    Ok(())
}
