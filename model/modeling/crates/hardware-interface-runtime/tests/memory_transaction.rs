#![allow(missing_docs, reason = "integration test crate")]
#![allow(clippy::expect_used, reason = "checked test fixture operations")]

mod support;

use std::sync::{Arc, Barrier};
use std::thread;

use hardware_interface_runtime::SharedMemory;
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
