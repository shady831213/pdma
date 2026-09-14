use crate::{Device, Error};
use hardware_interface_schema::{RegisterAccessError, register_dispatch};
use pdma_model_contract::{
    ChannelBlock, ChannelId, ChannelRegisters, CommandRegisters, DeviceControl, DeviceRegisters,
};

#[test]
fn channel_collection_miss_allows_control_in_either_order() -> Result<(), Box<dyn std::error::Error>>
{
    // Given: three channels and a control address outside their collection.
    let mut device = Device::new((), 3)?;
    let count = DeviceRegisters::CONTROL.offset() + DeviceControl::CHANNEL_NUM.offset();
    let enable = DeviceRegisters::CONTROL.offset() + DeviceControl::IRQ_ENABLE.offset();
    let mut out = [0; 4];
    // When: collection is attempted before control, the reverse of the public path.
    register_dispatch!(
        device.write_channels(enable, &1_u32.to_le_bytes()),
        device.write_control(enable, &1_u32.to_le_bytes()),
    )?;
    register_dispatch!(
        device.read_channels(count, &mut out),
        device
            .control
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
    let mut device = Device::new((), 3)?;
    for address in [
        0,
        DeviceRegisters::CHANNELS_OFFSET - 1,
        DeviceRegisters::extent(3)?,
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
    let mut device = Device::new((), 3)?;
    let offset = ChannelBlock::CONTROL.offset() + ChannelRegisters::IRQ_ENABLE.offset();
    let selected = ChannelId::new(2).register_offset().ok_or("offset")? + offset;
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
            ChannelId::new(index).register_offset().ok_or("offset")? + offset,
            &mut out,
        )?;
        assert_eq!(u32::from_le_bytes(out), u32::from(index == 2));
    }
    Ok(())
}

#[test]
fn collection_padding_is_terminal_inside_its_extent() -> Result<(), Box<dyn std::error::Error>> {
    // Given: padding between child maps and between adjacent channel blocks.
    let mut device = Device::new((), 3)?;
    for offset in [CommandRegisters::EXTENT, ChannelBlock::EXTENT] {
        let address = DeviceRegisters::CHANNELS_OFFSET + offset;
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
