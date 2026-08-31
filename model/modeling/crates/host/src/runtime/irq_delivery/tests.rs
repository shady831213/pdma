use super::{IrqHandlerRegistry, irq_handler};
use crate::runtime::interrupt::InterruptError;
use pdma_model_contract::semantic::IrqVector;

#[path = "../../../tests/irq_delivery.rs"]
mod behavior;

#[test]
fn empty_registry_returns_interrupt_error() {
    // Given: an interrupt registry with no handlers.
    let registry = IrqHandlerRegistry::new();

    // When: registration is frozen.
    let result = registry.freeze();

    // Then: the IRQ layer reports its owned empty-registry error.
    assert!(matches!(result, Err(InterruptError::EmptyRegistry)));
}

#[test]
fn duplicate_vector_returns_interrupt_error() {
    // Given: one vector already has a registered handler.
    let vector = IrqVector::new(7);
    let mut registry = IrqHandlerRegistry::new();
    let first = irq_handler(|_| async { Ok::<(), crate::DriverError>(()) });
    let second = irq_handler(|_| async { Ok::<(), crate::DriverError>(()) });
    assert!(registry.register(vector, first).is_ok());

    // When: a second handler targets the same vector.
    let result: Result<(), InterruptError> = registry.register(vector, second);

    // Then: the IRQ layer reports its owned duplicate-vector error.
    assert_eq!(result, Err(InterruptError::DuplicateVector(vector)));
}
