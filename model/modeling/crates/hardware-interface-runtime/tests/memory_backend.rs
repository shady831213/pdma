#![allow(missing_docs, reason = "integration test crate")]
#![allow(clippy::expect_used, reason = "checked test fixture operations")]

mod support;

use hardware_interface_runtime::{
    LocalMemoryStorage, Memory, MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment,
    MemoryError, MemoryRegion, SharedMemory,
};
use support::region;

#[derive(Debug, Eq, PartialEq)]
struct Transcript {
    regions: Vec<MemoryRegion>,
    bytes: [u8; 4],
    successes: Vec<bool>,
    errors: Vec<MemoryAccessError>,
}

trait MemoryFixture: MemoryAccess {
    fn installed_regions(&self) -> Vec<MemoryRegion>;
    fn install(&self, regions: &[MemoryRegion]) -> Result<(), MemoryError>;
    fn addition_error(&self, regions: &[MemoryRegion]) -> MemoryError;
}

impl MemoryFixture for SharedMemory {
    fn installed_regions(&self) -> Vec<MemoryRegion> {
        self.regions()
    }

    fn install(&self, regions: &[MemoryRegion]) -> Result<(), MemoryError> {
        self.add_regions(regions)
    }

    fn addition_error(&self, regions: &[MemoryRegion]) -> MemoryError {
        self.add_regions(regions).expect_err("addition is rejected")
    }
}

impl MemoryFixture for Memory<LocalMemoryStorage> {
    fn installed_regions(&self) -> Vec<MemoryRegion> {
        self.regions()
    }

    fn install(&self, regions: &[MemoryRegion]) -> Result<(), MemoryError> {
        self.add_regions(regions)
    }

    fn addition_error(&self, regions: &[MemoryRegion]) -> MemoryError {
        self.add_regions(regions).expect_err("addition is rejected")
    }
}

fn transcript(memory: &impl MemoryFixture) -> Transcript {
    let mapped = region(0x1000, 64, 64);
    let install = memory.install(&[mapped]);
    let write = memory.write(
        MemoryAddress::new(0x1000),
        MemoryAlignment::ONE,
        &[1, 2, 3, 4],
    );
    let mut bytes = [0; 4];
    let read = memory.read(MemoryAddress::new(0x1000), MemoryAlignment::ONE, &mut bytes);
    let endpoint = memory.read(MemoryAddress::new(0x1040), MemoryAlignment::ONE, &mut []);
    let overflow = memory
        .read(
            MemoryAddress::new(u64::MAX),
            MemoryAlignment::try_new(8).expect("alignment"),
            &mut [0; 2],
        )
        .expect_err("overflow precedes alignment and lookup");
    let unmapped = memory
        .read(MemoryAddress::new(0x2000), MemoryAlignment::ONE, &mut [0])
        .expect_err("unmapped start");
    Transcript {
        regions: memory.installed_regions(),
        bytes,
        successes: vec![
            install.is_ok(),
            write.is_ok(),
            read.is_ok(),
            endpoint.is_ok(),
        ],
        errors: vec![overflow, unmapped],
    }
}

fn adverse_errors(
    memory: &impl MemoryFixture,
) -> (MemoryError, MemoryError, Vec<MemoryAccessError>) {
    let mapped = region(0x1000, 64, 64);
    memory.install(&[mapped]).expect("initial region installs");
    let overlap = memory.addition_error(&[region(0x1020, 64, 32), region(0x2000, 64, 64)]);
    let capacity = memory.addition_error(&[region(0x2000, 64, 64)]);
    let misaligned = memory
        .read(
            MemoryAddress::new(0x1001),
            MemoryAlignment::try_new(8).expect("alignment"),
            &mut [0],
        )
        .expect_err("misaligned before lookup");
    let crossing = memory
        .read(
            MemoryAddress::new(0x103f),
            MemoryAlignment::ONE,
            &mut [0; 2],
        )
        .expect_err("crosses mapped region");
    (overlap, capacity, vec![misaligned, crossing])
}

#[test]
fn parity_local_and_default_execute_identical_literal_sequence() {
    // Given the selectable local policy and the compatibility facade.
    let local = Memory::<LocalMemoryStorage>::new();
    let shared = SharedMemory::new();
    let local_clone = local.clone();
    let shared_clone = shared.clone();

    // When both execute the same literal checked-memory sequence.
    let local_transcript = transcript(&local_clone);
    let shared_transcript = transcript(&shared_clone);

    // Then regions, bytes, results, and error precedence are identical.
    println!("local={local_transcript:?}");
    println!("default={shared_transcript:?}");
    assert_eq!(local_transcript, shared_transcript);
    assert_eq!(local.regions(), vec![region(0x1000, 64, 64)]);
    assert_eq!(shared.regions(), vec![region(0x1000, 64, 64)]);
    let mut local_bytes = [0; 4];
    let mut shared_bytes = [0; 4];
    local
        .read(
            MemoryAddress::new(0x1000),
            MemoryAlignment::ONE,
            &mut local_bytes,
        )
        .expect("original local handle reads clone writes");
    shared
        .read(
            MemoryAddress::new(0x1000),
            MemoryAlignment::ONE,
            &mut shared_bytes,
        )
        .expect("original shared handle reads clone writes");
    assert_eq!(local_bytes, [1, 2, 3, 4]);
    assert_eq!(shared_bytes, [1, 2, 3, 4]);
}

#[test]
fn parity_capacity_overlap_and_access_error_precedence() {
    // Given local and default memories with the same one-region capacity.
    let local = Memory::<LocalMemoryStorage>::with_region_capacity(1);
    let shared = SharedMemory::with_region_capacity(1);

    // When both execute the same overlapping, capacity, and access failures.
    let local_errors = adverse_errors(&local);
    let shared_errors = adverse_errors(&shared);

    // Then error variants, fields, and precedence are identical.
    println!("local_errors={local_errors:?}");
    println!("default_errors={shared_errors:?}");
    assert_eq!(local_errors, shared_errors);
}
