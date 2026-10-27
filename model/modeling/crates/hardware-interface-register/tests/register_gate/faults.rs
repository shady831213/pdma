use super::*;

#[test]
fn dropped_reservation_faults_gate_and_unblocks_followers() {
    // Given two reservations where the earlier committed holder will be dropped.
    let observed = Arc::new(Mutex::new(Vec::new()));
    let gate = ObservationGate::new(SharedRegisterObserver::new(Arc::new(SharedVecObserver(
        Arc::clone(&observed),
    ))));
    let abandoned = gate.reserve(2).expect("abandoned range");
    let follower = gate.reserve(1).expect("follower range");

    // When the earlier reservation is dropped and the follower attempts emission.
    drop(abandoned);
    let result = follower.emit(vec![write(2, AccessOrigin::Direct, 0x30, 0xcc)]);

    // Then the waiter wakes into a terminal fault without fabricating observations.
    let expected = ObservationGateError::Faulted {
        reason: hardware_interface_register::GateFault::ReservationDropped,
    };
    assert_eq!(result, Err(expected));
    assert_eq!(gate.reserve(1).expect_err("gate stays faulted"), expected);
    assert!(
        observed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_empty()
    );
}

#[test]
fn malformed_reservations_fail_closed_and_noop_accepts_observations() {
    // Given a gate at the final usable sequence with an infallible no-op observer.
    let gate = ObservationGate::new_at(
        SharedRegisterObserver::new(Arc::new(NoopRegisterObserver)),
        u64::MAX,
    );

    // When zero and overflowing ranges are requested, then validation precedes mutation.
    assert_eq!(
        gate.reserve(0).expect_err("zero rejected"),
        ObservationGateError::InvalidReservationCount
    );
    assert_eq!(
        gate.reserve(2).expect_err("overflow rejected"),
        ObservationGateError::SequenceExhausted {
            next_sequence: u64::MAX,
            count: 2,
        }
    );
    let final_item = gate.reserve(1).expect("final sequence");
    assert_eq!(final_item.start_sequence(), u64::MAX);
    final_item
        .emit(vec![write(u64::MAX, AccessOrigin::Direct, 0x10, 0xaa)])
        .expect("no-op emit");
}

#[test]
fn mismatched_emission_faults_gate_without_observation() {
    // Given one reserved sequence and a recording observer.
    let observed = Arc::new(Mutex::new(Vec::new()));
    let gate = ObservationGate::new(SharedRegisterObserver::new(Arc::new(SharedVecObserver(
        Arc::clone(&observed),
    ))));
    let reservation = gate.reserve(1).expect("one reservation");

    // When emission claims a different sequence than the committed reservation.
    let result = reservation.emit(vec![write(7, AccessOrigin::Direct, 0x10, 0xaa)]);

    // Then the mismatch terminally faults before invoking the observer.
    let expected = ObservationGateError::Faulted {
        reason: hardware_interface_register::GateFault::InvalidEmission,
    };
    assert_eq!(result, Err(expected));
    assert_eq!(gate.reserve(1).expect_err("gate stays faulted"), expected);
    assert!(
        observed
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .is_empty()
    );
}

#[derive(Debug)]
struct PanickingObserver;

impl RegisterObserver for PanickingObserver {
    fn observe(&self, _observation: RegisterObservation) {
        panic!("deliberate observer panic");
    }
}

#[test]
fn observer_panic_terminally_faults_gate_and_wakes_follower() {
    // Given earlier and later committed reservations sharing a panicking observer.
    let gate = ObservationGate::new(SharedRegisterObserver::new(Arc::new(PanickingObserver)));
    let earlier = gate.reserve(1).expect("earlier reservation");
    let follower = gate.reserve(1).expect("follower reservation");
    let start = Arc::new(Barrier::new(2));
    let follower_start = Arc::clone(&start);
    let follower_thread = thread::spawn(move || {
        follower_start.wait();
        follower.emit(vec![write(1, AccessOrigin::Direct, 0x20, 0xbb)])
    });

    // When the follower reaches emission and the earlier observer panics.
    start.wait();
    let earlier_result = earlier.emit(vec![write(0, AccessOrigin::Direct, 0x10, 0xaa)]);
    let follower_result = follower_thread.join().expect("follower joins");

    // Then both calls expose the same terminal observer fault and future work fails fast.
    let expected = ObservationGateError::Faulted {
        reason: hardware_interface_register::GateFault::ObserverPanicked,
    };
    assert_eq!(earlier_result, Err(expected));
    assert_eq!(follower_result, Err(expected));
    assert_eq!(gate.reserve(1).expect_err("gate stays faulted"), expected);
}
