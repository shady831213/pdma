#![allow(missing_docs, reason = "integration test crate")]
#![allow(clippy::expect_used, reason = "checked test fixture operations")]

use hw_model_io::register::observation::{
    GateFault, ObservationBackend, ObservationOrder, ObservationOrderError,
};

const fn assert_backend_contract<B: ObservationBackend>(_backend: &B) {}

#[test]
fn contiguous_reservations_advance_exactly_once() {
    // Given a fresh pure order with two contiguous committed ranges.
    let mut order = ObservationOrder::new();
    let mut batch = order.reserve(3).expect("batch reservation");
    let direct = order.reserve(1).expect("direct reservation");
    assert_eq!((batch.start_sequence(), direct.start_sequence()), (0, 3));

    // When the batch is validated and advanced once.
    let mut emission = order
        .validate(&mut batch, &[0, 1, 2])
        .expect("exact batch emission");
    order.advance(&mut emission).expect("batch advances");

    // Then the emission cursor advances exactly once without changing allocation.
    println!("reserved=[0..3,3..4] emitted=3 next_reserved=4");
    assert_eq!(order.next_emission_sequence(), Ok(3));
    assert_eq!(order.next_sequence(), Ok(4));
    assert_eq!(
        order.advance(&mut emission),
        Err(ObservationOrderError::Faulted {
            reason: GateFault::InvalidEmission,
        })
    );
    assert_eq!(
        order.next_emission_sequence(),
        Err(fault(GateFault::InvalidEmission))
    );
}

#[test]
fn later_range_cannot_advance_before_its_predecessor() {
    // Given adjacent earlier and later reservations with exact emissions.
    let mut order = ObservationOrder::new();
    let mut earlier = order.reserve(1).expect("earlier reservation");
    let mut later = order.reserve(2).expect("later reservation");
    let mut later_emission = order
        .validate(&mut later, &[1, 2])
        .expect("later range validates independently");

    // When the later emission tries to advance first.
    let result = order.advance(&mut later_emission);

    // Then it is rejected without advancing or consuming the receipt.
    println!("later_start=1 expected_start=0 cursor_unchanged=0");
    assert_eq!(
        result,
        Err(ObservationOrderError::EmissionOutOfOrder {
            expected_sequence: 0,
            actual_sequence: 1,
        })
    );
    assert_eq!(order.next_emission_sequence(), Ok(0));
    let mut earlier_emission = order
        .validate(&mut earlier, &[0])
        .expect("earlier validates");
    order
        .advance(&mut earlier_emission)
        .expect("earlier advances");
    order.advance(&mut later_emission).expect("later retries");
    assert_eq!(order.next_emission_sequence(), Ok(3));
}

#[test]
fn adverse_invalid_count_and_overflow_preserve_cursors() {
    // Given an order at the final usable sequence.
    let mut order = ObservationOrder::new_at(u64::MAX);

    // When malformed and overflowing reservations are attempted.
    let zero = order.reserve(0).expect_err("zero rejected");
    let overflow = order.reserve(2).expect_err("overflow rejected");

    // Then both exact boundary errors leave the order open and unchanged.
    println!(
        "zero={zero:?} overflow={overflow:?} cursor={:?}",
        order.next_sequence()
    );
    assert_eq!(zero, ObservationOrderError::InvalidReservationCount);
    assert_eq!(
        overflow,
        ObservationOrderError::SequenceExhausted {
            next_sequence: u64::MAX,
            count: 2,
        }
    );
    assert_eq!(order.next_sequence(), Ok(u64::MAX));
}

#[test]
fn adverse_invalid_sequence_faults_permanently_without_advancement() {
    // Given one reservation at the emission cursor.
    let mut order = ObservationOrder::new();
    let mut reservation = order.reserve(2).expect("range");

    // When validation supplies a wrong sequence.
    let result = order.validate(&mut reservation, &[0, 7]);

    // Then the invalid-emission fault is permanent and neither cursor advances again.
    assert_eq!(result, Err(fault(GateFault::InvalidEmission)));
    assert_permanent_fault(&mut order, GateFault::InvalidEmission, 1);
}

#[test]
fn adverse_invalid_range_length_faults_permanently() {
    // Given a two-item reservation.
    let mut order = ObservationOrder::new();
    let mut reservation = order.reserve(2).expect("range");

    // When validation supplies only one sequence.
    let result = order.validate(&mut reservation, &[0]);

    // Then validation faults before any emission advancement.
    assert_eq!(result, Err(fault(GateFault::InvalidEmission)));
    assert_permanent_fault(&mut order, GateFault::InvalidEmission, 1);
}

#[test]
fn adverse_explicit_abandonment_faults_permanently() {
    // Given a committed reservation and a later reserved range.
    let mut order = ObservationOrder::new();
    let mut abandoned = order.reserve(2).expect("abandoned range");
    let mut later = order.reserve(1).expect("later range");

    // When the earlier owner explicitly abandons its affine receipt.
    order.abandon(&mut abandoned).expect("abandonment recorded");

    // Then the pinned dropped-reservation fault rejects later validation and work.
    assert_eq!(
        order.validate(&mut later, &[2]),
        Err(fault(GateFault::ReservationDropped))
    );
    assert_permanent_fault(&mut order, GateFault::ReservationDropped, 1);
}

#[test]
fn adverse_double_validation_faults_permanently() {
    // Given a reservation already consumed into an emission receipt.
    let mut order = ObservationOrder::new();
    let mut reservation = order.reserve(1).expect("range");
    let _emission = order
        .validate(&mut reservation, &[0])
        .expect("first validation");

    // When the consumed reservation receipt is reused.
    let result = order.validate(&mut reservation, &[0]);

    // Then affine misuse closes the order without advancement.
    assert_eq!(result, Err(fault(GateFault::InvalidEmission)));
    assert_permanent_fault(&mut order, GateFault::InvalidEmission, 1);
}

#[test]
fn adverse_stale_emission_after_fault_cannot_advance() {
    // Given one validated emission and a separate live reservation.
    let mut order = ObservationOrder::new();
    let mut ready = order.reserve(1).expect("ready range");
    let mut stale = order.validate(&mut ready, &[0]).expect("ready emission");
    let mut abandoned = order.reserve(1).expect("abandoned range");
    order.abandon(&mut abandoned).expect("fault recorded");

    // When the pre-fault emission receipt attempts advancement.
    let result = order.advance(&mut stale);

    // Then the first fault wins and the cursor remains unchanged.
    assert_eq!(result, Err(fault(GateFault::ReservationDropped)));
    assert_permanent_fault(&mut order, GateFault::ReservationDropped, 1);
}

const fn fault(reason: GateFault) -> ObservationOrderError {
    ObservationOrderError::Faulted { reason }
}

fn assert_permanent_fault(order: &mut ObservationOrder, reason: GateFault, count: usize) {
    let expected = fault(reason);
    assert_eq!(order.next_emission_sequence(), Err(expected));
    assert_eq!(order.next_sequence(), Err(expected));
    assert_eq!(
        order.reserve(count).expect_err("later reserve rejected"),
        expected
    );
}

#[allow(dead_code, reason = "compile-time public backend contract pin")]
const fn backend_contract_is_runtime_neutral<B: ObservationBackend>(backend: &B) {
    assert_backend_contract(backend);
}
