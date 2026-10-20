//! Blocking policy reentry must fail instead of recursively acquiring its mutex.

use hardware_interface_runtime::{
    BlockingObservationBackend, GateFault, NoopRegisterObserver, ObservationBackend,
    ObservationGateError, SharedRegisterObserver,
};
use std::{
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, mpsc},
    thread,
    time::Duration,
};

type TestResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;

fn bounded(operation: impl FnOnce() -> TestResult + Send + 'static) -> TestResult {
    let (sender, receiver) = mpsc::channel();
    let worker = thread::spawn(move || sender.send(operation()));
    receiver.recv_timeout(Duration::from_secs(2))??;
    worker.join().map_err(|_| "probe thread panicked")??;
    Ok(())
}

fn backend() -> BlockingObservationBackend {
    BlockingObservationBackend::new(SharedRegisterObserver::new(Arc::new(NoopRegisterObserver)))
}

#[test]
fn nested_reservation_drop_returns_and_retains_fault() -> TestResult {
    bounded(|| {
        for mutable in [false, true] {
            let backend = backend();
            let reservation = backend.reserve(1)?;
            if mutable {
                ObservationBackend::mutate(&backend, |_| drop(reservation))?;
            } else {
                ObservationBackend::inspect(&backend, |_| drop(reservation))?;
            }
            assert_eq!(
                backend.next_sequence(),
                Err(ObservationGateError::Faulted {
                    reason: GateFault::ReservationDropped,
                })
            );
        }
        Ok(())
    })
}

#[test]
fn nested_gate_access_returns_permanent_error() -> TestResult {
    bounded(|| {
        let backend = backend();
        let result = ObservationBackend::inspect(&backend, |_| backend.next_sequence())?;
        assert_eq!(
            result,
            Err(ObservationGateError::Faulted {
                reason: GateFault::SynchronizationPoisoned,
            })
        );
        assert_eq!(backend.next_sequence(), result);
        Ok(())
    })
}

#[test]
fn abandonment_before_unwind_retains_first_fault() -> TestResult {
    bounded(|| {
        let backend = backend();
        let reservation = backend.reserve(1)?;
        let result = catch_unwind(AssertUnwindSafe(|| {
            ObservationBackend::inspect(&backend, |_| {
                drop(reservation);
                std::panic::resume_unwind(Box::new("interrupted after abandonment"));
            })
        }));
        assert!(result.is_err());
        assert_eq!(
            backend.next_sequence(),
            Err(ObservationGateError::Faulted {
                reason: GateFault::ReservationDropped,
            })
        );
        Ok(())
    })
}
