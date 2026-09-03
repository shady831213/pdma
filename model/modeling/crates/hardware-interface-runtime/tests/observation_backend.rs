#![allow(missing_docs, clippy::expect_used, clippy::panic)]

use std::cell::RefCell;
use std::panic::AssertUnwindSafe;
use std::sync::{
    Arc, Barrier, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread;

use hardware_interface_runtime::{
    AccessOrigin, BlockingObservationBackend, GateFault, LocalObservationBackend,
    ObservationGateError, RegisterObservation, RegisterObserver, SharedRegisterObserver,
};

#[derive(Debug, Default)]
struct Recorder {
    transcript: Mutex<Vec<RegisterObservation>>,
    callbacks: Mutex<usize>,
}

impl RegisterObserver for Recorder {
    fn observe(&self, observation: RegisterObservation) {
        self.transcript
            .lock()
            .expect("transcript lock")
            .push(observation);
        *self.callbacks.lock().expect("callback lock") += 1;
    }
}

fn write(sequence: u64, origin: AccessOrigin, offset: u32) -> RegisterObservation {
    RegisterObservation::write(sequence, origin, offset, &[0xa5])
}

fn literal_transcript(recorder: &Recorder) -> Vec<(u64, AccessOrigin, u32)> {
    recorder
        .transcript
        .lock()
        .expect("transcript lock")
        .iter()
        .map(|item| (item.sequence, item.origin, item.offset))
        .collect()
}

#[test]
fn parity_local_backend_emits_contiguous_transcript_before_return() {
    // Given a type-selected local backend and two contiguous reservations.
    let recorder = Arc::new(Recorder::default());
    let backend = LocalObservationBackend::new(SharedRegisterObserver::new(recorder.clone()));
    let batch = backend.reserve(2).expect("batch reservation");
    let direct = backend.reserve(1).expect("direct reservation");

    // When each range emits synchronously in reservation order.
    batch
        .emit(vec![
            write(0, AccessOrigin::Batch, 0x10),
            write(1, AccessOrigin::Batch, 0x20),
        ])
        .expect("batch emits");
    direct
        .emit(vec![write(2, AccessOrigin::Direct, 0x30)])
        .expect("direct emits");

    // Then the literal transcript is contiguous and all callbacks completed before return.
    let transcript = literal_transcript(&recorder);
    let callbacks = *recorder.callbacks.lock().expect("callback lock");
    println!("local transcript={transcript:?} callback_completed={callbacks}");
    assert_eq!(
        transcript,
        [
            (0, AccessOrigin::Batch, 0x10),
            (1, AccessOrigin::Batch, 0x20),
            (2, AccessOrigin::Direct, 0x30),
        ]
    );
    assert_eq!(callbacks, 3);
}

#[test]
fn parity_blocking_backend_emits_contiguous_transcript_before_return() {
    // Given the same operation sequence through the explicitly blocking type.
    let recorder = Arc::new(Recorder::default());
    let backend = BlockingObservationBackend::new(SharedRegisterObserver::new(recorder.clone()));
    let batch = backend.reserve(2).expect("batch reservation");
    let direct = backend.reserve(1).expect("direct reservation");

    // When each range emits synchronously in reservation order.
    batch
        .emit(vec![
            write(0, AccessOrigin::Batch, 0x10),
            write(1, AccessOrigin::Batch, 0x20),
        ])
        .expect("batch emits");
    direct
        .emit(vec![write(2, AccessOrigin::Direct, 0x30)])
        .expect("direct emits");

    // Then its literal transcript and callback-before-return marker match local policy.
    let transcript = literal_transcript(&recorder);
    let callbacks = *recorder.callbacks.lock().expect("callback lock");
    println!("blocking transcript={transcript:?} callback_completed={callbacks}");
    assert_eq!(
        transcript,
        [
            (0, AccessOrigin::Batch, 0x10),
            (1, AccessOrigin::Batch, 0x20),
            (2, AccessOrigin::Direct, 0x30),
        ]
    );
    assert_eq!(callbacks, 3);
}

thread_local! {
    static LOCAL_REENTRY: RefCell<Option<LocalObservationBackend>> = const { RefCell::new(None) };
    static LOCAL_NESTED_RESERVATION: RefCell<Option<hardware_interface_runtime::LocalObservationReservation>> = const { RefCell::new(None) };
}

#[derive(Debug, Default)]
struct LocalReentrantObserver(AtomicBool);

impl RegisterObserver for LocalReentrantObserver {
    fn observe(&self, _observation: RegisterObservation) {
        if !self.0.swap(true, Ordering::SeqCst) {
            LOCAL_REENTRY.with(|slot| {
                let backend = slot.borrow().clone().expect("local backend installed");
                assert_eq!(backend.next_sequence(), Ok(1));
                let independent = backend.reserve(1).expect("callback reservation");
                assert_eq!(independent.start_sequence(), 1);
                LOCAL_NESTED_RESERVATION.with(|nested| {
                    *nested.borrow_mut() = Some(independent);
                });
            });
        }
    }
}

#[test]
fn parity_local_callback_can_reenter_without_backend_borrow() {
    // Given a local observer that re-enters inspection and reservation.
    let backend = LocalObservationBackend::new(SharedRegisterObserver::new(Arc::new(
        LocalReentrantObserver::default(),
    )));
    LOCAL_REENTRY.with(|slot| *slot.borrow_mut() = Some(backend.clone()));
    let reservation = backend.reserve(1).expect("outer reservation");

    // When the callback re-enters, then no backend borrow remains held.
    reservation
        .emit(vec![write(0, AccessOrigin::Direct, 0x10)])
        .expect("outer emission");
    let nested = LOCAL_NESTED_RESERVATION
        .with(|slot| slot.borrow_mut().take())
        .expect("callback reservation retained");
    let result = nested.emit(vec![write(1, AccessOrigin::Direct, 0x20)]);
    LOCAL_REENTRY.with(|slot| slot.borrow_mut().take());
    println!("local callback_reentry={result:?}");
    assert_eq!(result, Ok(()));
}

#[derive(Debug, Default)]
struct BlockingReentrantObserver(Mutex<Option<BlockingObservationBackend>>);

impl RegisterObserver for BlockingReentrantObserver {
    fn observe(&self, _observation: RegisterObservation) {
        let backend = self
            .0
            .lock()
            .expect("observer lock")
            .clone()
            .expect("installed");
        assert_eq!(backend.next_sequence(), Ok(1));
    }
}

#[test]
fn parity_blocking_callback_can_reenter_without_backend_lock() {
    // Given a blocking observer that re-enters backend inspection.
    let observer = Arc::new(BlockingReentrantObserver::default());
    let backend = BlockingObservationBackend::new(SharedRegisterObserver::new(observer.clone()));
    *observer.0.lock().expect("observer lock") = Some(backend.clone());

    // When the callback re-enters, then emission completes without deadlock.
    let result =
        backend
            .reserve(1)
            .expect("reservation")
            .emit(vec![write(0, AccessOrigin::Direct, 0x10)]);
    println!("blocking callback_reentry={result:?}");
    assert_eq!(result, Ok(()));
}

#[derive(Debug)]
struct PanickingObserver;

impl RegisterObserver for PanickingObserver {
    fn observe(&self, _observation: RegisterObservation) {
        panic!("deliberate observer panic");
    }
}

#[test]
fn adverse_local_callback_panic_propagates_and_permanently_faults() {
    // Given a local backend whose observer panics.
    let backend =
        LocalObservationBackend::new(SharedRegisterObserver::new(Arc::new(PanickingObserver)));
    let reservation = backend.reserve(1).expect("reservation");

    // When callback panic propagation is caught by the test boundary.
    let panic_result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        reservation.emit(vec![write(0, AccessOrigin::Direct, 0x10)])
    }));

    // Then the first permanent fault is exact and allocation cannot advance.
    let expected = ObservationGateError::Faulted {
        reason: GateFault::ObserverPanicked,
    };
    println!(
        "local panic_propagated={} fault={expected:?}",
        panic_result.is_err()
    );
    assert!(panic_result.is_err());
    assert_eq!(backend.next_sequence(), Err(expected));
    assert_eq!(backend.reserve(1).expect_err("fault retained"), expected);
}

#[test]
fn adverse_blocking_callback_panic_is_caught_and_permanently_faults() {
    // Given the compatibility blocking policy with a panicking observer.
    let backend =
        BlockingObservationBackend::new(SharedRegisterObserver::new(Arc::new(PanickingObserver)));
    let reservation = backend.reserve(1).expect("reservation");

    // When callback emission runs, then the legacy typed result is returned exactly.
    let expected = ObservationGateError::Faulted {
        reason: GateFault::ObserverPanicked,
    };
    let result = reservation.emit(vec![write(0, AccessOrigin::Direct, 0x10)]);
    println!(
        "blocking panic_result={result:?} permanent={:?}",
        backend.next_sequence()
    );
    assert_eq!(result, Err(expected));
    assert_eq!(backend.next_sequence(), Err(expected));
}

#[test]
fn adverse_malformed_local_emission_preserves_first_fault() {
    // Given a local backend with one committed reservation.
    let backend =
        LocalObservationBackend::new(SharedRegisterObserver::new(Arc::new(Recorder::default())));

    // When its emission lies about the reserved sequence.
    let invalid = backend
        .reserve(1)
        .expect("malformed range")
        .emit(vec![write(9, AccessOrigin::Direct, 0x10)]);

    // Then the first fault is permanent without further allocation.
    let invalid_fault = ObservationGateError::Faulted {
        reason: GateFault::InvalidEmission,
    };
    println!(
        "local invalid={invalid:?} next={:?}",
        backend.next_sequence()
    );
    assert_eq!(invalid, Err(invalid_fault));
    assert_eq!(backend.next_sequence(), Err(invalid_fault));
}

#[test]
fn adverse_blocking_earlier_drop_wakes_waiting_follower() {
    // Given a later emitter that enters before its earlier reservation is dropped.
    let backend =
        BlockingObservationBackend::new(SharedRegisterObserver::new(Arc::new(Recorder::default())));
    let earlier = backend.reserve(1).expect("earlier range");
    let follower = backend.reserve(1).expect("follower range");
    let entered = Arc::new(Barrier::new(2));
    let follower_entered = Arc::clone(&entered);
    let (result_tx, result_rx) = mpsc::channel();
    let worker = thread::spawn(move || {
        follower_entered.wait();
        result_tx
            .send(follower.emit(vec![write(1, AccessOrigin::Direct, 0x20)]))
            .expect("result sent");
    });
    entered.wait();

    // When the earlier affine reservation is abandoned.
    drop(earlier);

    // Then the follower wakes with the exact permanent first fault.
    let expected = Err(ObservationGateError::Faulted {
        reason: GateFault::ReservationDropped,
    });
    let result = result_rx.recv().expect("follower result");
    worker.join().expect("follower joins");
    println!(
        "blocking dropped_follower={result:?} next={:?}",
        backend.next_sequence()
    );
    assert_eq!(result, expected);
    assert_eq!(backend.next_sequence(), expected.map(|()| 0));
}
