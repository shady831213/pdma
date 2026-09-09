//! Generated nested PDMA register placement.

use hardware_interface_schema::RegisterLayout;
use pdma_model_contract::{DeviceRegisters, DeviceRegistersValues};

#[test]
fn register_offsets_are_aligned_when_maps_are_nested() -> Result<(), Box<dyn std::error::Error>> {
    // Given: the generated device map with automatic placement.
    let map = DeviceRegisters::resolve()?;

    // When: nested register leaves are resolved.
    let leaves = map
        .registers()
        .iter()
        .map(|register| (register.path(), register.offset()))
        .collect::<Vec<_>>();

    // Then: queue maps align to eight bytes and the channel follows both queues.
    assert_eq!(
        leaves,
        [
            ("DeviceRegisters.command.base", 0),
            ("DeviceRegisters.command.capacity", 8),
            ("DeviceRegisters.command.producer", 12),
            ("DeviceRegisters.command.consumer", 16),
            ("DeviceRegisters.response.base", 24),
            ("DeviceRegisters.response.capacity", 32),
            ("DeviceRegisters.response.producer", 36),
            ("DeviceRegisters.response.consumer", 40),
            ("DeviceRegisters.channel.enable", 44),
            ("DeviceRegisters.channel.irq_enable", 48),
            ("DeviceRegisters.channel.fault", 52),
        ]
    );
    assert_eq!(DeviceRegisters::EXTENT, 56);
    assert_eq!(DeviceRegisters::ALIGNMENT, 8);
    Ok(())
}

#[test]
fn generated_access_updates_selected_leaf_when_registers_are_nested() {
    // Given: reset register storage with a visible sibling value.
    let mut values = DeviceRegistersValues::default();
    values.command.producer = pdma_model_contract::WordValues::new(9);

    // When: hardware updates the response producer through generated offset dispatch.
    let result = DeviceRegisters::write_register(&mut values, 36, 17);

    // Then: only the selected nested leaf changes.
    assert_eq!(result, Some(()));
    assert_eq!(values.response.producer.raw(), 17);
    assert_eq!(values.command.producer.raw(), 9);
    assert_eq!(DeviceRegisters::read_register(&values, 36), Some(17));
    assert_eq!(DeviceRegisters::read_register(&values, 37), None);
}
