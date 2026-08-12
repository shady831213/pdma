#![allow(missing_docs, reason = "integration test crate")]
#![allow(clippy::expect_used, reason = "checked test fixture operations")]
#![allow(clippy::panic, reason = "observer panic is the behavior under test")]

use hardware_interface_runtime::{
    AccessOrigin, NoopRegisterObserver, ObservationGate, ObservationGateError, RegisterObservation,
    RegisterObserver, SharedRegisterObserver,
};
use std::sync::{Arc, Barrier, Condvar, Mutex, mpsc};
use std::thread;

#[derive(Debug, Default)]
struct BlockingObserver {
    observations: Mutex<Vec<RegisterObservation>>,
    first_entered: Condvar,
    release_first: Condvar,
    state: Mutex<(bool, bool)>,
}

impl BlockingObserver {
    fn wait_first(&self) {
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        drop(
            self.first_entered
                .wait_while(state, |(entered, _)| !*entered)
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        );
    }

    fn release_first(&self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.1 = true;
        drop(state);
        self.release_first.notify_all();
    }
}

impl RegisterObserver for BlockingObserver {
    #[allow(
        clippy::significant_drop_tightening,
        reason = "the condition-variable predicate lock must span its wait"
    )]
    fn observe(&self, observation: RegisterObservation) {
        self.observations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(observation.clone());
        if observation.sequence == 0 {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state.0 = true;
            self.first_entered.notify_all();
            drop(
                self.release_first
                    .wait_while(state, |(_, released)| !*released)
                    .unwrap_or_else(std::sync::PoisonError::into_inner),
            );
        }
    }
}

fn write(sequence: u64, origin: AccessOrigin, offset: u32, byte: u8) -> RegisterObservation {
    RegisterObservation::write(sequence, origin, offset, &[byte])
}

#[test]
fn reversed_emitters_follow_reservation_order_and_wait_for_callbacks() {
    // Given two reservations made in commit order and a callback that blocks the first caller.
    let observer = Arc::new(BlockingObserver::default());
    let gate = ObservationGate::new(SharedRegisterObserver::new(observer.clone()));
    let first = gate.reserve(1).expect("first reservation");
    let second = gate.reserve(1).expect("second reservation");
    let reverse = Arc::new(Barrier::new(2));
    let (returned_tx, returned_rx) = mpsc::channel();

    // When the later reservation reaches emission before the earlier reservation.
    let later_barrier = Arc::clone(&reverse);
    let later_tx = returned_tx.clone();
    let later = thread::spawn(move || {
        later_barrier.wait();
        second
            .emit(vec![write(1, AccessOrigin::Direct, 0x20, 0xbb)])
            .expect("later emit");
        later_tx.send(1).expect("later return signal");
    });
    let earlier = thread::spawn(move || {
        reverse.wait();
        first
            .emit(vec![write(0, AccessOrigin::Direct, 0x10, 0xaa)])
            .expect("earlier emit");
        returned_tx.send(0).expect("earlier return signal");
    });
    observer.wait_first();
    assert!(returned_rx.try_recv().is_err());
    observer.release_first();

    // Then neither caller returns early and callbacks remain strictly sequenced.
    earlier.join().expect("earlier joins");
    later.join().expect("later joins");
    let mut returns = [
        returned_rx.recv().expect("one return"),
        returned_rx.recv().expect("other return"),
    ];
    returns.sort_unstable();
    assert_eq!(returns, [0, 1]);
    let sequences = observer
        .observations
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .iter()
        .map(|item| item.sequence)
        .collect::<Vec<_>>();
    assert_eq!(sequences, [0, 1]);
}

#[test]
fn direct_racing_batch_emits_batch_contiguously_in_commit_order() {
    // Given a three-item batch reservation followed by one direct reservation.
    let transcript = Arc::new(Mutex::new(Vec::new()));
    let recorder = SharedVecObserver(Arc::clone(&transcript));
    let gate = ObservationGate::new(SharedRegisterObserver::new(Arc::new(recorder)));
    let batch = gate.reserve(3).expect("batch range");
    let direct = gate.reserve(1).expect("direct range");
    let reverse = Arc::new(Barrier::new(2));

    // When direct reaches the gate first while batch arrives second.
    let direct_barrier = Arc::clone(&reverse);
    let direct_thread = thread::spawn(move || {
        direct_barrier.wait();
        direct
            .emit(vec![write(3, AccessOrigin::Direct, 0x40, 0xdd)])
            .expect("direct emit");
    });
    let batch_thread = thread::spawn(move || {
        reverse.wait();
        batch
            .emit(vec![
                write(0, AccessOrigin::Batch, 0x10, 0xa0),
                write(1, AccessOrigin::Batch, 0x20, 0xb0),
                write(2, AccessOrigin::Batch, 0x30, 0xc0),
            ])
            .expect("batch emit");
    });
    batch_thread.join().expect("batch joins");
    direct_thread.join().expect("direct joins");

    // Then commit order wins and every batch item stays adjacent.
    let snapshot = transcript
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    assert_eq!(
        snapshot
            .iter()
            .map(|item| (item.sequence, item.origin, item.offset))
            .collect::<Vec<_>>(),
        [
            (0, AccessOrigin::Batch, 0x10),
            (1, AccessOrigin::Batch, 0x20),
            (2, AccessOrigin::Batch, 0x30),
            (3, AccessOrigin::Direct, 0x40),
        ]
    );
}

#[derive(Debug)]
struct SharedVecObserver(Arc<Mutex<Vec<RegisterObservation>>>);

impl RegisterObserver for SharedVecObserver {
    fn observe(&self, observation: RegisterObservation) {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(observation);
    }
}

#[path = "register_gate/faults.rs"]
mod faults;
