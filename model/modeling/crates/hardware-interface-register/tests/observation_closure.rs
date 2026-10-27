//! Public closure reentry and independent gate contracts.
use hardware_interface_register::{
    BlockingObservationBackend, GateFault, ObservationBackend, ObservationGateError,
    SharedRegisterObserver,
};

#[test]
fn nested_closure_operations_fault_only_the_entered_gate() -> Result<(), ObservationGateError> {
    for nested in 0..3 {
        for inspect in [false, true] {
            let gate = BlockingObservationBackend::new(SharedRegisterObserver::new(
                std::sync::Arc::new(hardware_interface_register::NoopRegisterObserver),
            ));
            let clone = gate.clone();
            let operation = || match nested {
                0 => clone.inspect(|_| ()),
                1 => clone.mutate(|_| ()),
                _ => clone.reserve(1).map(drop),
            };
            let result = if inspect {
                gate.inspect(|_| operation())?
            } else {
                gate.mutate(|_| operation())?
            };
            let expected = ObservationGateError::Faulted {
                reason: GateFault::SynchronizationPoisoned,
            };
            assert_eq!(result, Err(expected));
            assert_eq!(gate.next_sequence(), Err(expected));
        }
    }
    Ok(())
}

#[test]
fn independent_gates_can_nest_closures_and_reservations() -> Result<(), ObservationGateError> {
    let outer = BlockingObservationBackend::new(SharedRegisterObserver::new(std::sync::Arc::new(
        hardware_interface_register::NoopRegisterObserver,
    )));
    let inner = BlockingObservationBackend::new(SharedRegisterObserver::new(std::sync::Arc::new(
        hardware_interface_register::NoopRegisterObserver,
    )));
    outer.inspect(|_| inner.mutate(|_| ()))??;
    outer.mutate(|_| inner.inspect(|_| ()))??;
    let reservation = outer.mutate(|_| inner.reserve(1))??;
    assert_eq!(reservation.start_sequence(), 0);
    reservation.cancel_latest()?;
    assert_eq!(outer.next_sequence(), Ok(0));
    assert_eq!(inner.next_sequence(), Ok(0));
    Ok(())
}
