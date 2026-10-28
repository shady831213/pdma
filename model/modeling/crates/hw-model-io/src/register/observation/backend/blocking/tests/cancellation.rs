use super::*;

#[test]
fn cancelling_latest_unused_reservation_reuses_its_sequence() {
    // Given: an earlier reservation with an unused latest successor.
    let gate = ObservationGate::new(SharedRegisterObserver::new(Arc::new(NoopRegisterObserver)));
    let earlier = gate.reserve(1).expect("earlier");
    let rejected = gate.reserve(2).expect("latest");

    // When: the latest operation rejects before committing any effect.
    rejected
        .cancel_latest()
        .expect("latest reservation cancelled");

    // Then: its sequence range is reusable and earlier emission remains valid.
    assert_eq!(gate.next_sequence(), Ok(1));
    earlier
        .emit(vec![RegisterObservation::write(
            0,
            crate::register::observation::AccessOrigin::Direct,
            0,
            &[1],
        )])
        .expect("earlier emits");
    let next = gate.reserve(1).expect("replacement");
    assert_eq!(next.start_sequence(), 1);
    next.emit(vec![RegisterObservation::write(
        1,
        crate::register::observation::AccessOrigin::Direct,
        0,
        &[2],
    )])
    .expect("replacement emits");
}

#[test]
fn cancelling_nonlatest_reservation_rejects_without_reusing_a_live_sequence() {
    // Given: a reservation followed by another committed observation slot.
    let gate = ObservationGate::new(SharedRegisterObserver::new(Arc::new(NoopRegisterObserver)));
    let earlier = gate.reserve(1).expect("earlier");
    let later = gate.reserve(1).expect("later");

    // When: a caller violates the required latest-reservation condition.
    let result = earlier.cancel_latest();

    // Then: the misuse closes the gate and cannot silently overwrite the later range.
    assert_eq!(
        result,
        Err(ObservationGateError::Faulted {
            reason: GateFault::InvalidEmission
        })
    );
    assert!(gate.reserve(1).is_err());
    drop(later);
}
