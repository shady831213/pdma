//! Fatal admission and configuration containment through the public bus.

use pdma_model_contract::dma::{
    ChannelBlock, ChannelId, ChannelRegisters, CommandRegisters, DmaControl, DmaDiscovery,
};
use pdma_model_dma::{Dma, Error};

/// Shared raw-bus fixtures.
pub mod support;

#[test]
fn active_copy_fatal_stops_next_memory_access_and_reset_retires_old_work() -> support::TestResult {
    use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment};
    use pdma_model_contract::dma::Endpoint;
    // Given: the first chunk of an accepted two-chunk command is externally visible.
    let (mut dma, memory) = support::setup(0x1100, 2)?;
    memory.write(MemoryAddress::new(0x1200), MemoryAlignment::ONE, &[7; 512])?;
    let mut command = support::command();
    command.copy_length = 512;
    command.source = Endpoint::linear(0x1200, 0);
    command.destination = Endpoint::linear(0x1600, 0);
    support::submit(&mut dma, &memory, command)?;
    let mut pipeline = dma.service();
    let mut operation = hardware_interface_pipeline::HardwareDriver::new();
    let mut prefix = [0; 256];
    for _ in 0..64 {
        assert!(matches!(
            operation.step(&mut pipeline, &mut dma),
            Ok(hardware_interface_pipeline::StepOutcome::InProgress)
        ));
        memory.read(
            MemoryAddress::new(0x1600),
            MemoryAlignment::ONE,
            &mut prefix,
        )?;
        if prefix == [7; 256] {
            break;
        }
    }
    assert_eq!(prefix, [7; 256]);
    // When: fatal interrupts before the next access and software requests whole-DMA cleanup.
    dma.signal_fatal();
    for _ in 0..32 {
        assert!(matches!(
            operation.step(&mut pipeline, &mut dma),
            Ok(hardware_interface_pipeline::StepOutcome::InProgress)
        ));
    }
    let reset = DmaDiscovery::EXTENT + DmaControl::RESET.offset();
    dma.write(reset, &1_u32.to_le_bytes())?;
    for _ in 0..64 {
        assert!(matches!(
            operation.step(&mut pipeline, &mut dma),
            Ok(hardware_interface_pipeline::StepOutcome::InProgress)
        ));
    }
    // Then: cleanup completes without rolling back the prefix or writing the old suffix.
    assert_eq!(support::read_word(&mut dma, reset)?, 0);
    memory.read(
        MemoryAddress::new(0x1600),
        MemoryAlignment::ONE,
        &mut prefix,
    )?;
    assert_eq!(prefix, [7; 256]);
    let mut suffix = [0; 256];
    memory.read(
        MemoryAddress::new(0x1700),
        MemoryAlignment::ONE,
        &mut suffix,
    )?;
    assert_eq!(suffix, [0; 256]);
    Ok(())
}

#[test]
fn fatal_rejects_new_channel_configuration_without_losing_status_or_reset()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: a configured notification source followed by a global hardware fault.
    let mut dma = Dma::new((), 2)?;
    let base = ChannelId::new(0).register_offset(2).ok_or("channel")?;
    let enabled = base + ChannelBlock::CONTROL.offset() + ChannelRegisters::IRQ_ENABLE.offset();
    dma.write(enabled, &1_u32.to_le_bytes())?;
    dma.signal_fatal();
    // When: another software transaction attempts to change data-plane configuration.
    let result = dma.write(enabled, &0_u32.to_le_bytes());
    // Then: hardware contains the race independently of firmware window closure.
    assert_eq!(result, Err(Error::Halted));
    let mut bytes = [0; 4];
    dma.read(enabled, &mut bytes)?;
    assert_eq!(u32::from_le_bytes(bytes), 1);
    dma.write(
        DmaDiscovery::EXTENT + DmaControl::RESET.offset(),
        &1_u32.to_le_bytes(),
    )?;
    Ok(())
}

#[test]
fn fatal_rejects_new_producer_publication() -> Result<(), Box<dyn std::error::Error>> {
    // Given: an idle command ring and a global fault.
    let mut dma = Dma::new((), 1)?;
    let base =
        ChannelId::new(0).register_offset(1).ok_or("channel")? + ChannelBlock::COMMAND.offset();
    dma.write(
        base + CommandRegisters::CAPACITY.offset(),
        &2_u32.to_le_bytes(),
    )?;
    dma.signal_fatal();
    // When: a submitter has not yet received PF's stop notification.
    let result = dma.write(
        base + CommandRegisters::PRODUCER.offset(),
        &1_u32.to_le_bytes(),
    );
    // Then: the producer stays unpublished until an explicit reset.
    assert_eq!(result, Err(Error::Halted));
    let mut bytes = [0; 4];
    dma.read(base + CommandRegisters::PRODUCER.offset(), &mut bytes)?;
    assert_eq!(u32::from_le_bytes(bytes), 0);
    Ok(())
}
