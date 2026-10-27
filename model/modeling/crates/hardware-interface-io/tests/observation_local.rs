//! Local observation ordering and retained failure boundaries.

use hardware_interface_io::register::observation::{
    AccessOrigin, GateFault, LocalObservationGate, NoopRegisterObserver, ObservationBackend,
    ObservationGateError, ObservationOrder, RegisterObservation, RegisterObserver,
    SharedRegisterObserver,
};
use std::cell::RefCell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, Mutex};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[derive(Default)]
struct Recorder(Mutex<Vec<u64>>);

impl RegisterObserver for Recorder {
    fn observe(&self, item: RegisterObservation) {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(item.sequence);
    }
}

const fn fault(reason: GateFault) -> ObservationGateError {
    ObservationGateError::Faulted { reason }
}

fn local() -> LocalObservationGate {
    LocalObservationGate::new(SharedRegisterObserver::new(Arc::new(NoopRegisterObserver)))
}

#[test]
fn later_emission_is_rejected_before_observer_callback() -> TestResult {
    let recorder = Arc::new(Recorder::default());
    let backend = LocalObservationGate::new(SharedRegisterObserver::new(recorder.clone()));
    let first = backend.reserve(1)?;
    let later = backend.reserve(1)?;
    let sequence = later.start_sequence();
    let result = later.emit(vec![RegisterObservation::write(
        sequence,
        AccessOrigin::Direct,
        0x10,
        &[1],
    )]);
    assert_eq!(result, Err(fault(GateFault::InvalidEmission)));
    assert!(recorder.0.lock().map_err(|e| e.to_string())?.is_empty());
    drop(first);
    assert_eq!(
        backend.next_sequence(),
        Err(fault(GateFault::InvalidEmission))
    );
    Ok(())
}

#[test]
fn reservation_drop_during_storage_access_retains_fault() -> TestResult {
    for mutable in [false, true] {
        let backend = local();
        let reservation = backend.reserve(1)?;
        if mutable {
            ObservationBackend::mutate(&backend, |_| drop(reservation))?;
        } else {
            ObservationBackend::inspect(&backend, |_| drop(reservation))?;
        }
        assert_eq!(
            backend.next_sequence(),
            Err(fault(GateFault::ReservationDropped))
        );
        assert_eq!(
            ObservationBackend::inspect(&backend, ObservationOrder::next_sequence)?,
            Err(
                hardware_interface_io::register::observation::ObservationOrderError::Faulted {
                    reason: GateFault::ReservationDropped,
                }
            )
        );
    }
    Ok(())
}

#[test]
fn reentrant_emission_rejection_is_permanent() -> TestResult {
    let backend = local();
    let reservation = backend.reserve(1)?;
    let sequence = reservation.start_sequence();
    let result = ObservationBackend::inspect(&backend, |_| {
        reservation.emit(vec![RegisterObservation::write(
            sequence,
            AccessOrigin::Direct,
            0x10,
            &[1],
        )])
    })?;
    assert_eq!(result, Err(fault(GateFault::SynchronizationPoisoned)));
    assert_eq!(backend.next_sequence(), result.map(|()| sequence));
    Ok(())
}

#[test]
fn unwind_after_reentrant_drop_retains_fault() -> TestResult {
    let backend = local();
    let reservation = backend.reserve(1)?;
    let result = catch_unwind(AssertUnwindSafe(|| {
        ObservationBackend::inspect(&backend, |_| {
            drop(reservation);
            std::panic::resume_unwind(Box::new("outer operation interrupted"));
        })
    }));
    assert!(result.is_err());
    assert_eq!(
        backend.next_sequence(),
        Err(fault(GateFault::ReservationDropped))
    );
    Ok(())
}

#[test]
fn already_recorded_fault_survives_reentrant_abandonment() -> TestResult {
    let backend = local();
    let reservation = backend.reserve(1)?;
    ObservationBackend::mutate(&backend, |order| {
        order.fault(GateFault::InvalidEmission);
        drop(reservation);
    })?;
    assert_eq!(
        backend.next_sequence(),
        Err(fault(GateFault::InvalidEmission))
    );
    Ok(())
}

#[test]
fn gate_access_rejection_matches_its_permanent_fault_result() -> TestResult {
    for reserve in [false, true] {
        let backend = local();
        let result = ObservationBackend::inspect(&backend, |_| {
            if reserve {
                backend.reserve(1).map(|_| ())
            } else {
                backend.next_sequence().map(|_| ())
            }
        })?;
        assert_eq!(result, Err(fault(GateFault::SynchronizationPoisoned)));
        assert_eq!(backend.next_sequence(), result.map(|()| 0));
    }
    Ok(())
}

thread_local! {
    static CALLBACK_RESERVATION: RefCell<Option<hardware_interface_io::register::observation::LocalObservationReservation>> =
        const { RefCell::new(None) };
}

#[derive(Default)]
struct AbandonObserver(Recorder);

impl RegisterObserver for AbandonObserver {
    fn observe(&self, item: RegisterObservation) {
        self.0.observe(item);
        CALLBACK_RESERVATION.with(|slot| drop(slot.borrow_mut().take()));
    }
}

#[test]
fn callback_abandonment_stops_remaining_batch_observations() -> TestResult {
    let observer = Arc::new(AbandonObserver::default());
    let backend = LocalObservationGate::new(SharedRegisterObserver::new(observer.clone()));
    let batch = backend.reserve(2)?;
    let start = batch.start_sequence();
    let later = backend.reserve(1)?;
    CALLBACK_RESERVATION.with(|slot| *slot.borrow_mut() = Some(later));
    let result = batch.emit(
        (start..start + 2)
            .map(|sequence| RegisterObservation::write(sequence, AccessOrigin::Batch, 0x10, &[1]))
            .collect(),
    );
    CALLBACK_RESERVATION.with(|slot| drop(slot.borrow_mut().take()));
    assert_eq!(result, Err(fault(GateFault::ReservationDropped)));
    assert_eq!(*observer.0.0.lock().map_err(|e| e.to_string())?, [start]);
    assert_eq!(
        backend.next_sequence(),
        Err(fault(GateFault::ReservationDropped))
    );
    Ok(())
}
