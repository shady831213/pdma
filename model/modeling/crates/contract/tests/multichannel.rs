//! Instance channel discovery and generated channel register storage.

use hardware_interface_schema::{
    RegisterLayout,
    register::{Access, ResetKind},
};
use pdma_model_contract::dma::{
    ChannelBlock, ChannelBlockValues, ChannelId, DmaDiscovery, DmaDiscoveryValues, DmaRegisters,
};

#[test]
fn channel_count_is_read_only_hardware_configuration_when_reset()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: a freshly reset device and its schema metadata.
    let values = DmaDiscoveryValues::default();
    let map = DmaRegisters::new(3)?.resolve()?;
    // When: software discovers the count at the fixed first register.
    let count = DmaDiscovery::read_register(&values, 0);
    let definition = map
        .registers()
        .first()
        .expect("discovery register")
        .definition();
    // Then: hardware must populate the read-only discovery value after reset.
    assert_eq!(count, Some(0));
    assert_eq!(definition.access(), Access::ReadOnly);
    assert_eq!(definition.writable_mask(), 0);
    assert_eq!(definition.reset_kind(), ResetKind::PostReset);
    assert_eq!(definition.reset_value(), 0);
    Ok(())
}

#[test]
fn channel_ids_follow_generated_stride_and_discovered_bounds() {
    // Given: valid selections and the first index beyond the current device.
    let selections = [0, 1, 2, 3].map(ChannelId::new);
    // When: translating selections into register block addresses.
    let addresses = selections.map(|id| id.register_offset(3));
    // Then: the selected instance bounds its page-aligned channel apertures.
    assert_eq!(selections.map(ChannelId::index), [0, 1, 2, 3]);
    assert_eq!(addresses, [Some(4096), Some(8192), Some(12288), None]);
    assert_eq!(ChannelId::new(usize::MAX).register_offset(1), None);
}

#[test]
fn generated_access_updates_only_selected_channel_when_dispatched() {
    // Given: reset register storage and command producers in separate channels.
    let mut channels = [ChannelBlockValues::default(); 3];
    let [first, middle, _] = &mut channels;
    assert_eq!(ChannelBlock::write_register(first, 12, 9), Some(()));
    assert_eq!(ChannelBlock::write_register(middle, 12, 11), Some(()));
    // When: hardware updates the middle channel's response producer.
    let result = ChannelBlock::write_register(middle, 36, 17);
    // Then: its siblings and both command producers retain their independent values.
    assert_eq!(result, Some(()));
    assert_eq!(
        channels.map(|channel| ChannelBlock::read_register(&channel, 12)),
        [Some(9), Some(11), Some(0)]
    );
    assert_eq!(
        channels.map(|channel| ChannelBlock::read_register(&channel, 36)),
        [Some(0), Some(17), Some(0)]
    );
}

#[test]
fn generated_dispatch_rejects_gaps_and_out_of_range_addresses() {
    // Given: reset register storage.
    let mut values = ChannelBlockValues::default();
    // When: accessing interiors, queue alignment gaps, channel padding and the map end.
    let results = [1, 20, 21, 22, 23, 73, 88, usize::MAX].map(|offset| {
        (
            ChannelBlock::read_register(&values, offset),
            ChannelBlock::write_register(&mut values, offset, 7),
        )
    });
    // Then: neither access path recognizes a leaf there.
    assert_eq!(results, [(None, None); 8]);
}
