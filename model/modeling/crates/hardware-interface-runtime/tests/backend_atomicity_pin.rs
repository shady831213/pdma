#![allow(missing_docs, reason = "integration test crate")]
#![allow(clippy::expect_used, reason = "checked test fixture operations")]

pub mod fixture;
mod support;

use std::future::Future;
use std::pin::Pin;
#[rustfmt::skip]
use std::sync::{Arc, Barrier, Mutex, atomic::{AtomicU64, Ordering}, mpsc};
use std::task::{Context, Poll, Waker};
use std::thread;

#[rustfmt::skip]
use hardware_interface_runtime::{AccessOrigin, GateFault, MemoryAccess, MemoryAddress, MemoryAlignment, MemoryError, ObservationGate, ObservationGateError, RegisterAccess, RegisterObservation, RegisterObserver, SharedMemory, SharedRegisterObserver};
use fixture::{PendingEndpoint, Recorder, write};
use support::region;

#[test]
fn competing_transaction_keeps_its_prepared_install_exclusive() {
    // Given an exclusive transaction and a clone-shared competing writer.
    let memory = SharedMemory::new();
    let competitor = memory.clone();
    let (attempted_tx, attempted_rx) = mpsc::channel();
    let writer = thread::spawn(move || {
        attempted_tx.send(()).expect("attempt signal");
        competitor
            .prepare_regions(&[region(0x2000, 64, 64)])
            .expect("prepare after exclusive transaction")
            .try_install(&competitor)
    });

    // When an owned result leaves the transaction while the competitor waits.
    let installed_revision = memory.transaction(|transaction| {
        let prepared = transaction
            .prepare_regions(&[region(0x1000, 64, 64)])
            .expect("transaction-local prepare");
        attempted_rx.recv().expect("competitor attempted access");
        transaction.install_prepared(prepared).raw()
    });
    let owned_result = thread::spawn(move || installed_revision)
        .join()
        .expect("owned transaction result crosses thread");

    // Then the branded transaction cannot escape, and both commits serialize.
    assert_eq!(
        (
            owned_result,
            writer.join().expect("competitor joins"),
            memory.revision().raw()
        ),
        (1, Ok(()), 2)
    );
}

#[test]
fn competing_same_revision_prepared_installs_have_one_winner() {
    // Given two independent proposals prepared against revision zero.
    let memory = SharedMemory::new();
    let first = memory
        .prepare_regions(&[region(0x1000, 64, 64)])
        .expect("first proposal");
    let second = memory
        .prepare_regions(&[region(0x2000, 64, 64)])
        .expect("second proposal");
    let start = Arc::new(Barrier::new(3));

    // When both proposal owners attempt installation concurrently.
    let workers = [(first, memory.clone()), (second, memory.clone())].map(|(proposal, store)| {
        let start = Arc::clone(&start);
        thread::spawn(move || {
            start.wait();
            proposal.try_install(&store)
        })
    });
    start.wait();
    let results = workers.map(|worker| worker.join().expect("installer joins"));

    // Then exactly one wins and exactly one observes the winner's revision.
    let stale = Err(MemoryError::StaleRevision {
        expected_revision: 0,
        actual_revision: 1,
    });
    assert!(matches!(
        &results,
        [Ok(()), error] | [error, Ok(())] if error == &stale
    ));
    assert_eq!(memory.revision().raw(), 1);
}

#[test]
fn cancellation_before_raw_register_commit_preserves_all_state() {
    // Given installed sentinel bytes and a raw write that suspends before commit.
    let memory = SharedMemory::new();
    memory
        .prepare_regions(&[region(0x1000, 64, 64)])
        .expect("region prepares")
        .try_install(&memory)
        .expect("region installs");
    memory
        .write(MemoryAddress::new(0x1000), MemoryAlignment::ONE, &[0x5a; 4])
        .expect("sentinel writes");
    let recorder = Arc::new(Recorder::default());
    let endpoint = PendingEndpoint::new(
        memory.clone(),
        ObservationGate::new(SharedRegisterObserver::new(recorder.clone())),
    );
    let before_revision = memory.revision();
    let mut future = Box::pin(endpoint.write(0x20, &[1, 2, 3, 4]));

    // When the future reports Pending and is dropped.
    let mut context = Context::from_waker(Waker::noop());
    assert!(matches!(
        Future::poll(Pin::as_mut(&mut future), &mut context),
        Poll::Pending
    ));
    drop(future);

    // Then bytes, revision, commit count, sequence, and transcript are unchanged.
    let mut bytes = [0; 4];
    memory
        .read(MemoryAddress::new(0x1000), MemoryAlignment::ONE, &mut bytes)
        .expect("sentinel reads");
    #[rustfmt::skip]
    assert_eq!((bytes, memory.revision(), endpoint.committed_writes.load(Ordering::SeqCst), endpoint.gate.next_sequence(), recorder.0.lock().expect("recorder lock").len()), ([0x5a; 4], before_revision, 0, Ok(0), 0));
}

#[derive(Debug)]
struct BlockingRecorder {
    observations: Mutex<Vec<RegisterObservation>>,
    entered: Arc<Barrier>,
    release: Arc<Barrier>,
}

impl RegisterObserver for BlockingRecorder {
    fn observe(&self, observation: RegisterObservation) {
        self.observations
            .lock()
            .expect("observation lock")
            .push(observation.clone());
        if observation.sequence == 0 {
            self.entered.wait();
            self.release.wait();
        }
    }
}

#[test]
fn competing_observations_emit_contiguously_in_reservation_order() {
    // Given an earlier batch whose first callback can be held active.
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let observer = Arc::new(BlockingRecorder {
        observations: Mutex::new(Vec::new()),
        entered: Arc::clone(&entered),
        release: Arc::clone(&release),
    });
    let gate = ObservationGate::new(SharedRegisterObserver::new(observer.clone()));
    let batch = gate.reserve(2).expect("batch reservation");
    let later = gate.reserve(1).expect("later reservation");

    // When the later emitter arrives while the earlier callback is active.
    let batch_thread = thread::spawn(move || {
        batch.emit(vec![
            write(0, AccessOrigin::Batch, 0x10),
            write(1, AccessOrigin::Batch, 0x20),
        ])
    });
    entered.wait();
    let (attempt_tx, attempt_rx) = mpsc::channel();
    let later_thread = thread::spawn(move || {
        attempt_tx.send(()).expect("later attempted");
        later.emit(vec![write(2, AccessOrigin::Direct, 0x30)])
    });
    attempt_rx.recv().expect("later reached emission call");
    assert_eq!(
        *observer.observations.lock().expect("observation lock"),
        [write(0, AccessOrigin::Batch, 0x10)]
    );
    release.wait();
    assert_eq!(batch_thread.join().expect("batch joins"), Ok(()));
    assert_eq!(later_thread.join().expect("later joins"), Ok(()));

    // Then reservation order wins and the batch remains contiguous.
    assert_eq!(
        *observer.observations.lock().expect("observation lock"),
        [
            write(0, AccessOrigin::Batch, 0x10),
            write(1, AccessOrigin::Batch, 0x20),
            write(2, AccessOrigin::Direct, 0x30),
        ]
    );
}

#[derive(Debug, Default)]
struct ReentrantObserver {
    gate: Mutex<Option<ObservationGate>>,
    observed_next: AtomicU64,
}

impl RegisterObserver for ReentrantObserver {
    fn observe(&self, _observation: RegisterObservation) {
        let gate = self
            .gate
            .lock()
            .expect("gate lock")
            .clone()
            .expect("gate installed");
        self.observed_next.store(
            gate.next_sequence().expect("callback can re-enter gate"),
            Ordering::SeqCst,
        );
    }
}

#[test]
fn callback_runs_after_gate_unlock_and_faults_are_permanent() {
    // Given a callback that re-enters public gate inspection.
    let observer = Arc::new(ReentrantObserver::default());
    let gate = ObservationGate::new(SharedRegisterObserver::new(observer.clone()));
    *observer.gate.lock().expect("gate lock") = Some(gate.clone());

    // When a valid observation emits, then callback re-entry completes without a guard escape.
    gate.reserve(1)
        .expect("valid reservation")
        .emit(vec![write(0, AccessOrigin::Direct, 0x10)])
        .expect("valid emission");
    assert_eq!(observer.observed_next.load(Ordering::SeqCst), 1);
    observer.gate.lock().expect("gate lock").take();

    // Given malformed emission and abandoned-reservation gates.
    let malformed =
        ObservationGate::new(SharedRegisterObserver::new(Arc::new(Recorder::default())));
    let malformed_result = malformed
        .reserve(1)
        .expect("malformed reservation")
        .emit(vec![write(7, AccessOrigin::Direct, 0x20)]);
    let dropped = ObservationGate::new(SharedRegisterObserver::new(Arc::new(Recorder::default())));
    drop(dropped.reserve(1).expect("abandoned reservation"));

    // Then first faults remain exact and permanent for all later operations.
    let invalid = ObservationGateError::Faulted {
        reason: GateFault::InvalidEmission,
    };
    let abandoned = ObservationGateError::Faulted {
        reason: GateFault::ReservationDropped,
    };
    assert_eq!(malformed_result, Err(invalid));
    assert_eq!(
        malformed.reserve(1).expect_err("invalid fault persists"),
        invalid
    );
    assert_eq!(dropped.next_sequence(), Err(abandoned));
    assert!(matches!(
        dropped.reserve(0),
        Err(ObservationGateError::InvalidReservationCount)
    ));
    assert_eq!(
        dropped.reserve(1).expect_err("drop fault persists"),
        abandoned
    );
}
