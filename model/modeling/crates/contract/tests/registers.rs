//! Generated nested PDMA register placement.

use hardware_interface_schema::{
    RegisterLayout,
    register::{Access, ResetKind},
};
use pdma_model_contract::{
    ChannelBlock, ChannelBlockValues, ChannelIrqMaskValues, ChannelRegisters, DeviceControl,
    DeviceRegisters,
};

#[test]
fn register_offsets_are_aligned_when_maps_are_nested() -> Result<(), Box<dyn std::error::Error>> {
    // Given: the generated device map with automatic placement.
    let map = DeviceRegisters::resolve(3)?;
    // When: nested register leaves are resolved.
    let leaves = map
        .registers()
        .iter()
        .map(|register| (register.path().to_owned(), register.offset()))
        .collect::<Vec<_>>();
    // Then: device discovery precedes three aligned, independent channel maps.
    let mut expected = [
        ("channel_num", 0),
        ("irq_enable", 4),
        ("fault", 8),
        ("reset", 12),
    ]
    .map(|(name, offset)| (format!("DeviceRegisters.control.{name}"), offset))
    .to_vec();
    for (index, base) in [(0, 16), (1, 88), (2, 160)] {
        expected.extend(
            [
                ("command.base", 0),
                ("command.capacity", 8),
                ("command.producer", 12),
                ("command.consumer", 16),
                ("response.base", 24),
                ("response.capacity", 32),
                ("response.producer", 36),
                ("response.consumer", 40),
                ("control.enable", 44),
                ("control.irq_enable", 48),
                ("control.fault", 52),
                ("control.irq_cause", 56),
                ("control.irq_mask", 60),
                ("control.reset", 64),
            ]
            .map(|(name, offset)| {
                (
                    format!("DeviceRegisters.channels[{index}].{name}"),
                    base + offset,
                )
            }),
        );
    }
    assert_eq!(leaves, expected);
    assert_eq!(DeviceRegisters::extent(3)?, 228);
    assert_eq!(DeviceRegisters::ALIGNMENT, 8);
    Ok(())
}

#[test]
fn generated_access_updates_selected_leaf_when_registers_are_nested() {
    // Given: reset register storage with a visible sibling value.
    let mut channel = ChannelBlockValues::default();
    channel.command.producer = pdma_model_contract::WordValues::new(9);
    // When: hardware updates the response producer through generated offset dispatch.
    let result = ChannelBlock::write_register(&mut channel, 36, 17);
    // Then: only the selected nested leaf changes.
    assert_eq!(result, Some(()));
    assert_eq!(channel.response.producer.raw(), 17);
    assert_eq!(channel.command.producer.raw(), 9);
    assert_eq!(ChannelBlock::read_register(&channel, 36), Some(17));
    assert_eq!(ChannelBlock::read_register(&channel, 37), None);
}

#[test]
fn interrupt_and_reset_metadata_define_software_access() -> Result<(), Box<dyn std::error::Error>> {
    // Given: the final nested channel register map.
    let map = ChannelRegisters::resolve()?;
    // When: software inspects the interrupt and recovery register definitions.
    let metadata = map
        .registers()
        .iter()
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
    // Then: causes are read-only, reset exposes busy state, and reserved bits are unwritable.
    assert_eq!(
        metadata,
        [
            (0, Access::ReadWrite, 1, ResetKind::Zero, 0),
            (4, Access::ReadWrite, 1, ResetKind::Zero, 0),
            (8, Access::ReadOnly, 0, ResetKind::Zero, 0),
            (12, Access::ReadOnly, 0, ResetKind::Zero, 0),
            (16, Access::ReadWrite, 3, ResetKind::Zero, 0),
            (20, Access::ReadWrite, 1, ResetKind::Zero, 0),
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

#[test]
fn device_recovery_metadata_defines_independent_software_access()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: device-wide control registers independent of each channel.
    let map = DeviceControl::resolve()?;
    // When: software inspects interrupt and recovery metadata after discovery.
    let metadata = map
        .registers()
        .iter()
        .skip(1)
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
    // Then: device fatal is read-only and device reset exposes busy state with bit zero writable.
    assert_eq!(
        metadata,
        [
            (4, Access::ReadWrite, 1, ResetKind::Zero, 0),
            (8, Access::ReadOnly, 0, ResetKind::Zero, 0),
            (12, Access::ReadWrite, 1, ResetKind::Zero, 0),
        ]
    );
    Ok(())
}
