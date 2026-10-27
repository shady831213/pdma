use super::*;
use crate::Dma;
use hardware_interface_memory::{MemoryAddress, MemoryAlignment, MemoryRegion, SharedMemory};
use pdma_model_contract::dma::ChannelInterrupts;
use pdma_model_contract::dma::{ChannelRegisters, EventRegisters};
use pdma_model_contract::dma::{ErrorInfo, EventInfoValues};
use pdma_model_contract::dma::{Event, EventCode};
use pdma_model_contract::register::{ReadWordValues, WordValues};

#[test]
fn ring_mmio_uses_its_instance_base() -> Result<(), Box<dyn std::error::Error>> {
    // Given: independently placed command and event register windows.
    let mut command = CommandRing::new(0x200);
    let mut event = EventRing::new(0x100);
    // When: each ring accepts a write at its own placement.
    command.write(0x208, &4_u32.to_le_bytes(), false, false)?;
    event.write(0x108, &8_u32.to_le_bytes(), false, false)?;
    // Then: generated offsets select distinct ring-owned values.
    let mut bytes = [0; 4];
    command.read(0x208, &mut bytes)?;
    assert_eq!(u32::from_le_bytes(bytes), 4);
    event.read(0x108, &mut bytes)?;
    assert_eq!(u32::from_le_bytes(bytes), 8);
    Ok(())
}

#[test]
fn consuming_events_across_counter_wrap_tracks_remaining_occupancy()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: a valid empty ring just before the 32-bit cursor wrap.
    let memory = SharedMemory::new();
    memory.add_regions(&[MemoryRegion::try_new(
        MemoryAddress::new(0x1000),
        u64::try_from(4 * Event::RECORD_SIZE)?,
        MemoryAlignment::ONE,
    )?])?;
    let mut device = Dma::new(memory, 1)?;
    device.write(
        pdma_model_contract::dma::ChannelBlock::ALIGNMENT
            + pdma_model_contract::dma::ChannelBlock::EVENT.offset()
            + EventRegisters::BASE.offset(),
        &0x1000_u64.to_le_bytes(),
    )?;
    device.write(
        pdma_model_contract::dma::ChannelBlock::ALIGNMENT
            + pdma_model_contract::dma::ChannelBlock::EVENT.offset()
            + EventRegisters::CAPACITY.offset(),
        &4_u32.to_le_bytes(),
    )?;
    device
        .channels
        .get_mut(0)
        .ok_or("missing channel")?
        .event
        .registers
        .values
        .producer = ReadWordValues::new(u32::MAX - 1);
    device
        .channels
        .get_mut(0)
        .ok_or("missing channel")?
        .event
        .registers
        .values
        .consumer = WordValues::new(u32::MAX - 1);
    for id in 0..3 {
        let event = Event {
            id,
            code: EventCode::ReadError,
            info: EventInfoValues::ReadError(ErrorInfo { bytes_completed: 0 }),
        };
        assert_eq!(
            device
                .channels
                .get_mut(0)
                .ok_or("missing channel")?
                .event
                .push(&device.memory, &event),
            Ok(())
        );
    }
    device
        .channels
        .get_mut(0)
        .ok_or("missing channel")?
        .update_irq();
    // When: consumer crosses each entry, including the boundary at zero.
    for (consumer, expected) in [(u32::MAX, 1_u32), (0, 1), (1, 0)] {
        device.write(
            pdma_model_contract::dma::ChannelBlock::ALIGNMENT
                + pdma_model_contract::dma::ChannelBlock::EVENT.offset()
                + EventRegisters::CONSUMER.offset(),
            &consumer.to_le_bytes(),
        )?;
        // Then: non-empty follows the actual remaining entries.
        let mut bytes = [0; 4];
        device.read(
            pdma_model_contract::dma::ChannelBlock::ALIGNMENT
                + pdma_model_contract::dma::ChannelBlock::CONTROL.offset()
                + ChannelRegisters::IRQ.offset()
                + ChannelInterrupts::CAUSE.offset(),
            &mut bytes,
        )?;
        assert_eq!(u32::from_le_bytes(bytes), expected);
    }
    Ok(())
}
