#![allow(missing_docs, reason = "integration test crate")]
#![allow(clippy::expect_used, reason = "checked test fixture operations")]

use std::sync::{Arc, Barrier, mpsc};
use std::thread;

use hardware_interface_runtime::{LocalStorage, MutexStorage, StorageBackend, StorageError};

fn parity_transcript<B: StorageBackend<Vec<&'static str>>>(backend: &B) -> Vec<&'static str> {
    backend
        .mutate(|values| {
            values.push("prepared");
            values.push("committed");
        })
        .expect("mutation succeeds");
    backend.inspect(Clone::clone).expect("inspection succeeds")
}

#[test]
fn parity_is_identical_for_local_and_mutex_backends() {
    // Given local and shared backends with identical values.
    let local = LocalStorage::new(vec!["initial"]);
    let shared = MutexStorage::new(vec!["initial"]);

    // When the same generic operation mutates each backend.
    let transcripts = (parity_transcript(&local), parity_transcript(&shared));

    // Then both produce the exact same observable transcript.
    println!("local={:?} shared={:?}", transcripts.0, transcripts.1);
    assert_eq!(
        transcripts,
        (
            vec!["initial", "prepared", "committed"],
            vec!["initial", "prepared", "committed"]
        )
    );
}

#[test]
fn clones_share_their_backend_value() {
    // Given clones of both backend policies.
    let local = LocalStorage::new(1_u64);
    let local_clone = local.clone();
    let shared = MutexStorage::new(10_u64);
    let shared_clone = shared.clone();

    // When each clone mutates its value.
    local_clone
        .mutate(|value| *value += 1)
        .expect("local clone mutates");
    shared_clone
        .mutate(|value| *value += 5)
        .expect("shared clone mutates");

    // Then the original handles observe the clone-owned mutations.
    assert_eq!(local.inspect(|value| *value), Ok(2));
    assert_eq!(shared.inspect(|value| *value), Ok(15));
}

#[test]
fn parity_shared_mutations_are_serialized_exactly_once() {
    // Given workers synchronized to contend for one shared backend.
    const WORKERS: usize = 8;
    const MUTATIONS: usize = 64;
    let shared = MutexStorage::new((0_usize, Vec::new()));
    let start = Arc::new(Barrier::new(WORKERS + 1));
    let workers: Vec<_> = (0..WORKERS)
        .map(|worker| {
            let shared = shared.clone();
            let start = Arc::clone(&start);
            thread::spawn(move || {
                start.wait();
                for turn in 0..MUTATIONS {
                    shared
                        .mutate(|state| {
                            state.0 += 1;
                            state.1.push((worker, turn));
                        })
                        .expect("serialized mutation");
                }
            })
        })
        .collect();
    start.wait();

    // When every contending worker completes.
    for worker in workers {
        worker.join().expect("worker joins");
    }

    // Then each call contributed one complete serialized mutation.
    let final_state = shared.inspect(Clone::clone).expect("final state");
    println!(
        "serialized_count={} transcript_count={}",
        final_state.0,
        final_state.1.len()
    );
    assert_eq!(final_state.0, WORKERS * MUTATIONS);
    assert_eq!(final_state.1.len(), WORKERS * MUTATIONS);
}

#[test]
fn ordinary_cross_thread_contention_blocks_then_completes() {
    // Given one thread holding a shared mutation while another attempts access.
    let shared = MutexStorage::new(1_u64);
    let holder = shared.clone();
    let contender = shared.clone();
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let (attempted_tx, attempted_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let entered_worker = Arc::clone(&entered);
    let release_worker = Arc::clone(&release);
    let first = thread::spawn(move || {
        holder.mutate(|value| {
            entered_worker.wait();
            release_worker.wait();
            *value += 1;
        })
    });
    entered.wait();
    let second = thread::spawn(move || {
        attempted_tx.send(()).expect("attempt signaled");
        let result = contender.mutate(|value| *value += 1);
        done_tx.send(result).expect("completion signaled");
    });

    // When contention is observed before the holder releases the lock.
    attempted_rx.recv().expect("contender attempted");
    assert!(done_rx.try_recv().is_err());
    release.wait();

    // Then the contender completes normally rather than reporting reentry.
    assert_eq!(done_rx.recv().expect("contender completes"), Ok(()));
    assert_eq!(first.join().expect("holder joins"), Ok(()));
    second.join().expect("contender joins");
    assert_eq!(shared.inspect(|value| *value), Ok(3));
}

fn assert_reentrant_matrix<B: StorageBackend<u64> + Clone>(storage: &B, initial: u64) {
    let nested = storage.clone();
    assert_eq!(
        storage.inspect(|_| nested.inspect(|value| *value)),
        Ok(Err(StorageError::Reentrant))
    );
    assert_eq!(
        storage.inspect(|_| nested.mutate(|value| *value = 99)),
        Ok(Err(StorageError::Reentrant))
    );
    assert_eq!(
        storage.mutate(|_| nested.inspect(|value| *value)),
        Ok(Err(StorageError::Reentrant))
    );
    assert_eq!(
        storage.mutate(|_| nested.mutate(|value| *value = 99)),
        Ok(Err(StorageError::Reentrant))
    );
    assert_eq!(storage.inspect(|value| *value), Ok(initial));
}

#[test]
fn reentrant_local_access_is_rejected_without_mutation() {
    // Given a local backend and a clone used for nested access.
    let storage = LocalStorage::new(7_u64);

    // When every nested inspect/mutate combination is attempted.
    assert_reentrant_matrix(&storage, 7);

    // Then each nested closure was rejected and the value stayed unchanged.
    println!("local_reentrant=Reentrant value=7");
}

#[test]
fn reentrant_shared_access_is_rejected_without_deadlock_or_mutation() {
    // Given a shared backend and a clone used for nested access.
    let storage = MutexStorage::new(11_u64);

    // When every nested inspect/mutate combination is attempted.
    assert_reentrant_matrix(&storage, 11);

    // Then each nested closure was rejected and the value stayed unchanged.
    println!("shared_reentrant=Reentrant value=11");
}

#[test]
fn different_backends_may_be_nested() {
    // Given two independent shared backends.
    let outer = MutexStorage::new(3_u64);
    let inner = MutexStorage::new(4_u64);

    // When one backend is mutated from the other's closure.
    let result = outer.mutate(|outer_value| {
        inner.mutate(|inner_value| {
            *outer_value += 1;
            *inner_value += 1;
        })
    });

    // Then both mutations commit normally.
    assert_eq!(result, Ok(Ok(())));
    assert_eq!(outer.inspect(|value| *value), Ok(4));
    assert_eq!(inner.inspect(|value| *value), Ok(5));
}

#[test]
fn poisoned_shared_storage_recovers_and_retains_mutation() {
    // Given a shared backend whose mutation changes the value before panicking.
    let storage = MutexStorage::new(20_u64);
    let poisoned = storage.clone();
    let panic_result = thread::spawn(move || {
        let _ = poisoned.mutate(|value| {
            *value = 21;
            panic!("poison test");
        });
    })
    .join();
    assert!(panic_result.is_err());

    // When a later call mutates the poisoned backend.
    let result = storage.mutate(|value| *value += 1);

    // Then poison recovery retained and mutated the protected value.
    assert_eq!(result, Ok(()));
    assert_eq!(storage.inspect(|value| *value), Ok(22));
}
