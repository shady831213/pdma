use super::*;
use crate::register::observation::{AccessOrigin, NoopRegisterObserver};
use std::sync::Mutex;
use std::thread;

mod cancellation;
mod emission;
mod reentry;

fn wait_for_one_waiter(backend: &ObservationGate) {
    let state = backend.state.lock().expect("backend lock");
    drop(
        backend
            .changed
            .wait_while(state, |state| state.waiting_emitters == 0)
            .expect("waiter visible"),
    );
}

#[test]
// CLIPPY-ALLOW: deliberate panic is required to exercise poisoned backend recovery.
#[allow(clippy::panic, reason = "mutex poisoning is the behavior under test")]
fn synchronization_poison_terminally_faults_backend() {
    let backend = ObservationGate::new(SharedRegisterObserver::new(Arc::new(NoopRegisterObserver)));
    let poison = backend.clone();
    let panic = std::panic::catch_unwind(AssertUnwindSafe(move || {
        let _guard = poison.state.lock().expect("initial lock");
        panic!("deliberate backend poison");
    }));

    assert!(panic.is_err());
    let expected = Err(ObservationGateError::Faulted {
        reason: GateFault::SynchronizationPoisoned,
    });
    assert_eq!(backend.next_sequence(), expected);
    assert_eq!(backend.next_sequence(), expected);
}

#[test]
fn earlier_drop_wakes_an_already_blocked_follower() {
    let backend = ObservationGate::new(SharedRegisterObserver::new(Arc::new(NoopRegisterObserver)));
    let earlier = backend.reserve(1).expect("earlier reservation");
    let follower = backend.reserve(1).expect("follower reservation");
    let worker = thread::spawn(move || {
        follower.emit(vec![RegisterObservation::write(
            1,
            AccessOrigin::Direct,
            0x20,
            &[0xbb],
        )])
    });
    wait_for_one_waiter(&backend);

    drop(earlier);

    assert_eq!(
        worker.join().expect("follower joins"),
        Err(ObservationGateError::Faulted {
            reason: GateFault::ReservationDropped,
        })
    );
}
