use super::*;
use crate::Dma;
use hardware_interface_runtime::{MemoryAddress, MemoryAlignment, MemoryRegion, SharedMemory};
use pdma_model_contract::{
    ChannelRegisters, CompletionFlagsValues, ReadWordValues, ResponseRegisters, WordValues,
};

#[test]
fn ring_mmio_uses_its_instance_base() -> Result<(), Box<dyn std::error::Error>> {
    // Given: independently placed command and response register windows.
    let mut command = CommandRing::new(0x200);
    let mut response = ResponseRing::new(0x100);
    // When: each ring accepts a write at its own placement.
    command.write(0x208, &4_u32.to_le_bytes(), false, false)?;
    response.write(0x108, &8_u32.to_le_bytes(), false, false)?;
    // Then: generated offsets select distinct ring-owned values.
    let mut bytes = [0; 4];
    command.read(0x208, &mut bytes)?;
    assert_eq!(u32::from_le_bytes(bytes), 4);
    response.read(0x108, &mut bytes)?;
    assert_eq!(u32::from_le_bytes(bytes), 8);
    Ok(())
}

#[test]
fn consuming_multiple_iocs_across_counter_wrap_clears_only_consumed_causes()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: a valid empty ring just before the 32-bit cursor wrap.
    let memory = SharedMemory::new();
    memory.add_regions(&[MemoryRegion::try_new(
        MemoryAddress::new(0x1000),
        256,
        MemoryAlignment::ONE,
    )?])?;
    let mut device = Dma::new(memory, 1)?;
    device.write(
        pdma_model_contract::ChannelBlock::ALIGNMENT
            + pdma_model_contract::ChannelBlock::RESPONSE.offset()
            + ResponseRegisters::BASE.offset(),
        &0x1000_u64.to_le_bytes(),
    )?;
    device.write(
        pdma_model_contract::ChannelBlock::ALIGNMENT
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
        .values
        .producer = ReadWordValues::new(u32::MAX - 1);
    device
        .channels
        .get_mut(0)
        .ok_or("missing channel")?
        .response
        .registers
        .values
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
            pdma_model_contract::ChannelBlock::ALIGNMENT
                + pdma_model_contract::ChannelBlock::RESPONSE.offset()
                + ResponseRegisters::CONSUMER.offset(),
            &consumer.to_le_bytes(),
        )?;
        // Then: non-empty and IOC causes follow the actual remaining entries.
        let mut bytes = [0; 4];
        device.read(
            pdma_model_contract::ChannelBlock::ALIGNMENT
                + pdma_model_contract::ChannelBlock::CONTROL.offset()
                + ChannelRegisters::IRQ_CAUSE.offset(),
            &mut bytes,
        )?;
        assert_eq!(u32::from_le_bytes(bytes), expected);
    }
    Ok(())
}
