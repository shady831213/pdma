#![allow(missing_docs, reason = "integration test crate")]
#![allow(clippy::expect_used, reason = "checked test fixture operations")]

mod support;

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Barrier};
use std::thread;

use hardware_interface_runtime::{
    LocalMemoryStorage, Memory, MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment,
    MemoryError, MemoryRegion, SharedMemory,
};
use support::region;

#[derive(Debug, Eq, PartialEq)]
struct Transcript {
    regions: Vec<MemoryRegion>,
    revisions: Vec<u64>,
    bytes: [u8; 4],
    successes: Vec<bool>,
    errors: Vec<MemoryAccessError>,
}

trait MemoryFixture: MemoryAccess {
    fn revision_raw(&self) -> u64;
    fn installed_regions(&self) -> Vec<MemoryRegion>;
    fn install(&self, regions: &[MemoryRegion]) -> Result<(), MemoryError>;
    fn prepare_error(&self, regions: &[MemoryRegion]) -> MemoryError;
}

impl MemoryFixture for SharedMemory {
    fn revision_raw(&self) -> u64 {
        self.revision().raw()
    }

    fn installed_regions(&self) -> Vec<MemoryRegion> {
        self.regions()
    }

    fn install(&self, regions: &[MemoryRegion]) -> Result<(), MemoryError> {
        self.prepare_regions(regions)?.try_install(self)
    }

    fn prepare_error(&self, regions: &[MemoryRegion]) -> MemoryError {
        self.prepare_regions(regions)
            .expect_err("proposal is rejected")
    }
}

impl MemoryFixture for Memory<LocalMemoryStorage> {
    fn revision_raw(&self) -> u64 {
        self.revision().raw()
    }

    fn installed_regions(&self) -> Vec<MemoryRegion> {
        self.regions()
    }

    fn install(&self, regions: &[MemoryRegion]) -> Result<(), MemoryError> {
        self.prepare_regions(regions)?.try_install_on(self)
    }

    fn prepare_error(&self, regions: &[MemoryRegion]) -> MemoryError {
        self.prepare_regions(regions)
            .expect_err("proposal is rejected")
    }
}

fn transcript(memory: &impl MemoryFixture) -> Transcript {
    let mapped = region(0x1000, 64, 64);
    let install = memory.install(&[mapped]);
    let first_revision = memory.revision_raw();
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
        revisions: vec![0, first_revision, memory.revision_raw()],
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
    let overlap = memory.prepare_error(&[region(0x1020, 64, 32), region(0x2000, 64, 64)]);
    let capacity = memory.prepare_error(&[region(0x2000, 64, 64)]);
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

    // Then regions, revisions, bytes, results, and error precedence are identical.
    println!("local={local_transcript:?}");
    println!("default={shared_transcript:?}");
    assert_eq!(local_transcript, shared_transcript);
    assert_eq!(local.revision().raw(), 1);
    assert_eq!(shared.revision().raw(), 1);
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

#[test]
fn adverse_stale_and_dropped_prospective_state_commit_nothing() {
    // Given local memory with two same-revision proposals and a draft that will drop.
    let memory = Memory::<LocalMemoryStorage>::new();
    let stale = memory
        .prepare_regions(&[region(0x1000, 64, 64)])
        .expect("stale proposal");
    let winner = memory
        .prepare_regions(&[region(0x2000, 64, 64)])
        .expect("winner proposal");
    let dropped = memory
        .prepare_regions(&[region(0x4000, 64, 64)])
        .expect("dropped proposal");
    drop(dropped);
    winner.try_install_on(&memory).expect("winner installs");
    let before_regions = memory.regions();
    let before_revision = memory.revision();

    // When stale installation fails and an unwinding transaction drops its draft.
    let stale_error = stale.try_install_on(&memory);
    let unwind = catch_unwind(AssertUnwindSafe(|| {
        memory.transaction(|transaction| {
            let mut draft = transaction.try_draft().expect("draft clones");
            draft
                .prepare_regions(&[region(0x3000, 64, 64)])
                .expect("draft prepares");
            draft
                .write(MemoryAddress::new(0x2000), MemoryAlignment::ONE, &[0xaa])
                .expect("draft writes");
            panic!("drop prospective state");
        });
    }));

    // Then neither prospective state changes regions, revision, or committed bytes.
    let mut committed = [0];
    memory
        .read(
            MemoryAddress::new(0x2000),
            MemoryAlignment::ONE,
            &mut committed,
        )
        .expect("committed byte reads");
    println!(
        "stale={stale_error:?};revision={:?};regions={:?};bytes={committed:?}",
        memory.revision(),
        memory.regions()
    );
    assert_eq!(
        stale_error,
        Err(MemoryError::StaleRevision {
            expected_revision: 0,
            actual_revision: 1,
        })
    );
    assert!(unwind.is_err());
    assert_eq!(memory.regions(), before_regions);
    assert_eq!(memory.revision(), before_revision);
    assert_eq!(committed, [0]);
}

#[test]
fn adverse_default_recovers_after_unwinding_storage_closure() {
    // Given default memory whose transaction will poison its mutex while unwinding.
    let memory = SharedMemory::new();

    // When the transaction unwinds before installing its prepared proposal.
    let unwind = catch_unwind(AssertUnwindSafe(|| {
        memory.transaction(|transaction| {
            let _prepared = transaction
                .prepare_regions(&[region(0x1000, 64, 64)])
                .expect("proposal prepares");
            panic!("poison default storage");
        });
    }));

    // Then poison recovery exposes unchanged state and accepts a later commit.
    let install = memory
        .prepare_regions(&[region(0x2000, 64, 64)])
        .and_then(|prepared| prepared.try_install(&memory));
    println!(
        "unwind={};install={install:?};revision={:?};regions={:?}",
        unwind.is_err(),
        memory.revision(),
        memory.regions()
    );
    assert!(unwind.is_err());
    assert_eq!(install, Ok(()));
    assert_eq!(memory.revision().raw(), 1);
    assert_eq!(memory.regions(), vec![region(0x2000, 64, 64)]);
}

#[test]
fn competing_writer_serializes_behind_default_transaction() {
    // Given a shared transaction and one writer synchronized without timing.
    let memory = SharedMemory::new();
    let competitor = memory.clone();
    let barrier = Arc::new(Barrier::new(2));
    let released = Arc::clone(&barrier);
    let writer = thread::spawn(move || {
        released.wait();
        competitor
            .prepare_regions(&[region(0x2000, 64, 64)])
            .and_then(|prepared| prepared.try_install(&competitor))
    });

    // When the transaction commits before releasing its exclusive view.
    let first = memory.transaction(|transaction| {
        let prepared = transaction
            .prepare_regions(&[region(0x1000, 64, 64)])
            .expect("transaction prepares");
        barrier.wait();
        transaction.install_prepared(prepared)
    });
    let second = writer.join().expect("writer joins");

    // Then the competing writer commits at the following revision.
    assert_eq!(first.raw(), 1);
    assert_eq!(second, Ok(()));
    assert_eq!(memory.revision().raw(), 2);
}
