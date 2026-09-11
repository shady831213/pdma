//! Generated nested PDMA register placement.

use hardware_interface_schema::{
    RegisterLayout,
    register::{Access, ResetKind},
};
use pdma_model_contract::{ChannelIrqMaskValues, DeviceRegisters, DeviceRegistersValues};

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
            ("DeviceRegisters.channel.irq_cause", 56),
            ("DeviceRegisters.channel.irq_mask", 60),
            ("DeviceRegisters.channel.reset", 64),
            ("DeviceRegisters.control.irq_enable", 68),
            ("DeviceRegisters.control.fault", 72),
            ("DeviceRegisters.control.reset", 76),
        ]
    );
    assert_eq!(DeviceRegisters::EXTENT, 80);
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

#[test]
fn interrupt_and_reset_metadata_define_software_access() -> Result<(), Box<dyn std::error::Error>> {
    // Given: the final nested register map.
    let map = DeviceRegisters::resolve()?;

    // When: software inspects the interrupt and recovery register definitions.
    let metadata = map
        .registers()
        .iter()
        .filter(|register| register.offset() >= 52)
        .map(|register| {
            let definition = register.definition();
            (
                register.offset(),
                definition.access(),
                definition.writable_mask(),
                definition.reset_kind(),
                definition.reset_value(),
            )
        })
        .collect::<Vec<_>>();

    // Then: causes are read-only, reset requests are write-only, and reserved bits are unwritable.
    assert_eq!(
        metadata,
        [
            (52, Access::ReadOnly, 0, ResetKind::Zero, 0),
            (56, Access::ReadOnly, 0, ResetKind::Zero, 0),
            (60, Access::ReadWrite, 3, ResetKind::Zero, 0),
            (64, Access::WriteOnly, 1, ResetKind::Zero, 0),
            (68, Access::ReadWrite, 1, ResetKind::Zero, 0),
            (72, Access::ReadOnly, 0, ResetKind::Zero, 0),
            (76, Access::WriteOnly, 1, ResetKind::Zero, 0),
        ]
    );
    Ok(())
}

#[test]
fn response_notification_masks_have_independent_wire_bits() -> Result<(), Box<dyn std::error::Error>>
{
    // Given: reset notification masks.
    let masks = ChannelIrqMaskValues::default();

    // When: software masks each response notification source.
    let non_empty = masks.with_response_non_empty(true)?;
    let ioc = masks.with_ioc(true)?;

    // Then: each source occupies its declared bit with reserved bits clear.
    assert_eq!(non_empty.raw(), 1);
    assert_eq!(ioc.raw(), 2);
    Ok(())
}
