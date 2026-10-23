//! Generated nested PDMA register placement.

use hardware_interface_schema::{
    RegisterLayout,
    register::{Access, ResetKind},
};
use pdma_model_contract::dma::{
    ChannelBlock, ChannelBlockValues, ChannelIrqMaskValues, ChannelRegisters, DmaControl,
    DmaRegisters, EventRegisters,
};

#[test]
fn register_offsets_are_aligned_when_maps_are_nested() -> Result<(), Box<dyn std::error::Error>> {
    // Given: the generated device map with automatic placement.
    let map = DmaRegisters::new(3)?.resolve()?;
    // When: nested register leaves are resolved.
    let leaves = map
        .registers()
        .iter()
        .map(|register| (register.path().to_owned(), register.offset()))
        .collect::<Vec<_>>();
    // Then: device discovery precedes three aligned, independent channel maps.
    let mut expected = vec![
        ("DmaRegisters.discovery.channel_num".to_owned(), 0),
        ("DmaRegisters.discovery.capabilities".to_owned(), 4),
    ];
    expected.extend(
        [("irq_enable", 8), ("fatal", 12), ("reset", 16)]
            .map(|(name, offset)| (format!("DmaRegisters.admin.control.{name}"), offset)),
    );
    expected.extend((0..3).map(|index| {
        (
            format!("DmaRegisters.admin.channels[{index}].reset"),
            20 + index * 4,
        )
    }));
    for (index, base) in [(0, 4096), (1, 8192), (2, 12288)] {
        expected.extend(
            [
                ("command.base", 0),
                ("command.capacity", 8),
                ("command.producer", 12),
                ("command.consumer", 16),
                ("event.base", 24),
                ("event.capacity", 32),
                ("event.producer", 36),
                ("event.consumer", 40),
                ("control.config.enable", 44),
                ("control.config.ring_context", 48),
                ("control.recovery.fatal", 52),
                ("control.recovery.fault_status", 56),
                ("control.recovery.resume", 60),
                ("control.irq.enable", 64),
                ("control.irq.cause", 68),
                ("control.irq.mask", 72),
                ("control.irq.trap_ack", 76),
            ]
            .map(|(name, offset)| {
                (
                    format!("DmaRegisters.channels[{index}].{name}"),
                    base + offset,
                )
            }),
        );
    }
    assert_eq!(leaves, expected);
    assert_eq!(DmaRegisters::new(3)?.extent(), 16384);
    assert_eq!(DmaRegisters::ALIGNMENT, 4096);
    Ok(())
}

#[test]
fn generated_access_updates_selected_leaf_when_registers_are_nested() {
    // Given: reset register storage with a visible sibling value.
    let mut channel = ChannelBlockValues::default();
    let command_producer = 9;
    let response_producer = 17;
    let response_offset = ChannelBlock::EVENT.offset() + EventRegisters::PRODUCER.offset();
    channel.command.producer = pdma_model_contract::register::WordValues::new(command_producer);
    // When: hardware updates the response producer through generated offset dispatch.
    let result =
        ChannelBlock::write_register(&mut channel, response_offset, u64::from(response_producer));
    // Then: only the selected nested leaf changes.
    assert_eq!(result, Some(()));
    assert_eq!(channel.event.producer.raw(), response_producer);
    assert_eq!(channel.command.producer.raw(), command_producer);
    assert_eq!(
        ChannelBlock::read_register(&channel, response_offset),
        Some(u64::from(response_producer))
    );
    assert_eq!(
        ChannelBlock::read_register(&channel, response_offset + 1),
        None
    );
}

#[test]
fn channel_control_metadata_defines_software_access() -> Result<(), Box<dyn std::error::Error>> {
    // Given: the final nested channel register map.
    let map = ChannelRegisters::resolve()?;
    // When: software inspects the channel interrupt register definitions.
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
    // Then: causes are read-only and reserved bits are unwritable.
    assert_eq!(
        metadata,
        [
            (0, Access::ReadWrite, 1, ResetKind::Zero, 0),
            (4, Access::ReadWrite, 0xffff_ffff, ResetKind::Zero, 0),
            (8, Access::ReadOnly, 0, ResetKind::Zero, 0),
            (12, Access::ReadOnly, 0, ResetKind::Zero, 0),
            (16, Access::WriteOnly, 1, ResetKind::Zero, 0),
            (20, Access::ReadWrite, 1, ResetKind::Zero, 0),
            (24, Access::ReadOnly, 0, ResetKind::Zero, 0),
            (28, Access::ReadWrite, 3, ResetKind::Zero, 0),
            (32, Access::WriteOnly, 1, ResetKind::Zero, 0),
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
    let non_empty = masks.with_event_non_empty(true)?;
    let ioc = masks.with_trap(true)?;
    // Then: each source occupies its declared bit with reserved bits clear.
    assert_eq!(non_empty.raw(), 1);
    assert_eq!(ioc.raw(), 2);
    Ok(())
}

#[test]
fn device_recovery_metadata_defines_independent_software_access()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: device-wide control registers independent of each channel.
    let map = DmaControl::resolve()?;
    // When: software inspects interrupt and recovery metadata after discovery.
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
    // Then: device fatal is read-only and device reset exposes busy state with bit zero writable.
    assert_eq!(
        metadata,
        [
            (0, Access::ReadWrite, 1, ResetKind::Zero, 0),
            (4, Access::ReadOnly, 0, ResetKind::Zero, 0),
            (8, Access::ReadWrite, 1, ResetKind::Zero, 0),
        ]
    );
    Ok(())
}
