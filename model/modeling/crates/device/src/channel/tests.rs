use super::*;
use crate::Device;
use hardware_interface_runtime::{MemoryRegion, SharedMemory};
use pdma_model_contract::{
    ChannelRegisters, CompletionFlagsValues, DeviceRegisters, ResponseRegisters, WordValues,
};

#[test]
fn consuming_multiple_iocs_across_counter_wrap_clears_only_consumed_causes()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: a valid empty ring just before the 32-bit cursor wrap.
    let memory = SharedMemory::new();
    memory
        .prepare_regions(&[MemoryRegion::try_new(
            MemoryAddress::new(0x1000),
            256,
            MemoryAlignment::ONE,
        )?])?
        .try_install(&memory)?;
    let mut device = Device::new(memory, 1)?;
    device.write(
        DeviceRegisters::CHANNELS_OFFSET
            + pdma_model_contract::ChannelBlock::RESPONSE.offset()
            + ResponseRegisters::BASE.offset(),
        &0x1000_u64.to_le_bytes(),
    )?;
    device.write(
        DeviceRegisters::CHANNELS_OFFSET
            + pdma_model_contract::ChannelBlock::RESPONSE.offset()
            + ResponseRegisters::CAPACITY.offset(),
        &4_u32.to_le_bytes(),
    )?;
    device
        .channels
        .get_mut(0)
        .ok_or("missing channel")?
        .response
        .registers
        .producer = ReadWordValues::new(u32::MAX - 1);
    device
        .channels
        .get_mut(0)
        .ok_or("missing channel")?
        .response
        .registers
        .consumer = WordValues::new(u32::MAX - 1);
    for ioc in [true, true, false] {
        let response = Response {
            id: 7,
            status: ResponseStatus::Success,
            flags: CompletionFlagsValues { ioc },
            bytes_completed: 0,
        };
        assert_eq!(
            device
                .channels
                .get_mut(0)
                .ok_or("missing channel")?
                .response
                .push(&device.memory, &response),
            Ok(true)
        );
    }
    device
        .channels
        .get_mut(0)
        .ok_or("missing channel")?
        .update_irq();
    // When: consumer crosses each IOC, including the boundary at zero.
    for (consumer, expected) in [(u32::MAX, 3_u32), (0, 1), (1, 0)] {
        device.write(
            DeviceRegisters::CHANNELS_OFFSET
                + pdma_model_contract::ChannelBlock::RESPONSE.offset()
                + ResponseRegisters::CONSUMER.offset(),
            &consumer.to_le_bytes(),
        )?;
        // Then: non-empty and IOC causes follow the actual remaining entries.
        let mut bytes = [0; 4];
        device.read(
            DeviceRegisters::CHANNELS_OFFSET
                + pdma_model_contract::ChannelBlock::CONTROL.offset()
                + ChannelRegisters::IRQ_CAUSE.offset(),
            &mut bytes,
        )?;
        assert_eq!(u32::from_le_bytes(bytes), expected);
    }
    Ok(())
}
