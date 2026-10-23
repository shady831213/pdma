//! Approved managed-register layout boundaries.
use pdma_model_contract::hw_components::{
    context::{ContextManagementRegisters, ContextTableRegisters},
    control::ControlRegisters,
};

#[test]
fn context_header_is_dense_and_keeps_atomic_request_aligned()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: the compact context header and a table covering three bitmap words.
    let map = ContextManagementRegisters::resolve()?;
    // When: the schema resolves the management leaves and table banks.
    let leaves = map
        .registers()
        .iter()
        .map(|r| (r.path(), r.offset()))
        .collect::<Vec<_>>();
    // Then: the result fills the alignment gap before the atomic request.
    assert_eq!(
        leaves,
        [
            ("ContextManagementRegisters.capacity", 0),
            ("ContextManagementRegisters.result", 4),
            ("ContextManagementRegisters.request", 8),
        ]
    );
    assert_eq!(ContextManagementRegisters::EXTENT, 16);
    let table = ContextTableRegisters::for_channels(3, 65)?;
    assert_eq!(table.entries().base(), 16);
    assert_eq!(table.failed().base(), 64);
    assert_eq!(table.failed().end(), table.extent());
    Ok(())
}

#[test]
fn control_configuration_publishes_after_attributes() -> Result<(), Box<dyn std::error::Error>> {
    // Given: one PF configuration group preceding independent doorbells.
    let map = ControlRegisters::resolve()?;
    // When: all physical leaves are resolved in publication order.
    let leaves = map
        .registers()
        .iter()
        .map(|r| (r.path(), r.offset()))
        .collect::<Vec<_>>();
    // Then: enable follows its configuration, and notification offsets stay fixed.
    assert_eq!(
        leaves,
        [
            ("ControlRegisters.config.bootstrap", 0),
            ("ControlRegisters.config.attributes.address_space", 8),
            ("ControlRegisters.config.attributes.pasid", 12),
            ("ControlRegisters.config.enable", 16),
            ("ControlRegisters.request", 20),
            ("ControlRegisters.notification.response", 24),
            ("ControlRegisters.notification.event", 28),
            ("ControlRegisters.notification.event_error", 32),
        ]
    );
    assert_eq!(ControlRegisters::EXTENT, 36);
    Ok(())
}
