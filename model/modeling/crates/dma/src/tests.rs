use crate::{Dma, Error};
use hardware_interface_schema::{RegisterAccessError, register_dispatch};
use pdma_model_contract::dma::ChannelInterrupts;
use pdma_model_contract::dma::{
    ChannelBlock, ChannelId, ChannelRegisters, CommandRegisters, DmaControl, DmaRegisters,
};

#[test]
fn nested_admin_bank_preserves_selection_and_absolute_misses()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: administration crosses the first page in a 1020-channel instance.
    let mut device = Dma::new((), 1020)?;
    // When: the last channel's reset is requested through its administrative aperture.
    device.write_admin(4096, &1_u32.to_le_bytes())?;
    // Then: only that channel reset, and misses retain the root address and output.
    for (index, channel) in device.channels.iter().enumerate() {
        assert_eq!(channel.generation, u64::from(index == 1019));
    }
    for (address, expected) in [(20, 0), (4092, 0), (4096, 0)] {
        let mut out = [0; 4];
        device.read_admin(address, &mut out)?;
        assert_eq!(u32::from_le_bytes(out), expected);
    }
    for address in [0, 4, 16, 4100, 8192, usize::MAX] {
        let mut out = [0xa5; 4];
        assert_eq!(
            device.read_admin(address, &mut out),
            Err(RegisterAccessError::OutsideWindow { address })
        );
        assert_eq!(
            device.write_admin(address, &1_u32.to_le_bytes()),
            Err(RegisterAccessError::OutsideWindow { address })
        );
        assert_eq!(out, [0xa5; 4]);
    }
    Ok(())
}

#[test]
fn channel_collection_miss_allows_control_in_either_order() -> Result<(), Box<dyn std::error::Error>>
{
    // Given: three channels and a control address outside their collection.
    let mut device = Dma::new((), 3)?;
    let count = DmaRegisters::DISCOVERY.offset()
        + pdma_model_contract::dma::DmaDiscovery::CHANNEL_NUM.offset();
    let enable = (pdma_model_contract::dma::DmaDiscovery::EXTENT
        + pdma_model_contract::dma::DmaAdmin::CONTROL.offset())
        + DmaControl::IRQ_ENABLE.offset();
    let mut out = [0; 4];
    // When: collection is attempted before control, the reverse of the public path.
    register_dispatch!(
        device.write_channels(enable, &1_u32.to_le_bytes()),
        device.write_control(enable, &1_u32.to_le_bytes()),
    )?;
    register_dispatch!(
        device.read_channels(count, &mut out),
        device
            .discovery
            .read(count, &mut out)
            .map_err(RegisterAccessError::widen::<Error>),
    )?;
    // Then: control remains reachable with the same value in both orders.
    assert_eq!(u32::from_le_bytes(out), 3);
    device.read(count, &mut out)?;
    assert_eq!(u32::from_le_bytes(out), 3);
    device.signal_fatal();
    assert!(device.take_interrupt());
    Ok(())
}

#[test]
fn channel_collection_preserves_miss_addresses_and_output() -> Result<(), Box<dyn std::error::Error>>
{
    // Given: an instance with three channels and sentinel output.
    let mut device = Dma::new((), 3)?;
    for address in [
        0,
        pdma_model_contract::dma::ChannelBlock::ALIGNMENT - 1,
        DmaRegisters::new(3)?.extent(),
        usize::MAX,
    ] {
        let mut out = [0xa5; 4];
        // When: an address lies outside the repeated channel window.
        let read = device.read_channels(address, &mut out);
        let write = device.write_channels(address, &1_u32.to_le_bytes());
        // Then: the original address is retained and no access took place.
        assert_eq!(read, Err(RegisterAccessError::OutsideWindow { address }));
        assert_eq!(write, Err(RegisterAccessError::OutsideWindow { address }));
        assert_eq!(out, [0xa5; 4]);
    }
    Ok(())
}

#[test]
fn collection_indexes_channels_and_preserves_terminal_errors()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: each channel has a distinct IRQ-enable state.
    let mut device = Dma::new((), 3)?;
    let offset = ChannelBlock::CONTROL.offset()
        + ChannelRegisters::IRQ.offset()
        + ChannelInterrupts::ENABLE.offset();
    let selected = ChannelId::new(2).register_offset(3).ok_or("offset")? + offset;
    device.write(selected, &1_u32.to_le_bytes())?;
    // When: accesses are dispatched through the repeated window.
    let rejected = device.write_channels(selected, &2_u32.to_le_bytes());
    // Then: a hit's mask error is terminal; sibling registers remain independent.
    assert!(matches!(
        rejected,
        Err(RegisterAccessError::ReservedBitChange { changed: 2, .. })
    ));
    for index in 0..3 {
        let mut out = [0; 4];
        device.read(
            ChannelId::new(index).register_offset(3).ok_or("offset")? + offset,
            &mut out,
        )?;
        assert_eq!(u32::from_le_bytes(out), u32::from(index == 2));
    }
    Ok(())
}

#[test]
fn collection_padding_is_terminal_inside_its_extent() -> Result<(), Box<dyn std::error::Error>> {
    // Given: padding between child maps and between adjacent channel blocks.
    let mut device = Dma::new((), 3)?;
    for offset in [CommandRegisters::EXTENT, CommandRegisters::EXTENT + 1] {
        let address = pdma_model_contract::dma::ChannelBlock::ALIGNMENT + offset;
        let mut out = [0xa5; 4];
        // When: an access hits padding inside the channel collection.
        let read = device.read_channels(address, &mut out);
        let write = device.write_channels(address, &1_u32.to_le_bytes());
        // Then: the collection owns the gap, so this is not a dispatch miss.
        assert_eq!(read, Err(RegisterAccessError::InvalidOffset { offset }));
        assert_eq!(write, Err(RegisterAccessError::InvalidOffset { offset }));
        assert_eq!(out, [0xa5; 4]);
    }
    Ok(())
}
