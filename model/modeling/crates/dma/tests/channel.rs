//! Raw bus, copy publication and channel-fault behavior.

use pdma_model_contract::dma::{
    ChannelInterrupts, ChannelRecovery, ChannelRegisters, Command, CommandRegisters,
    CopyCommandSourceValues, Event, EventCode, EventRegisters, LinearEndpoint,
};

/// Shared raw-bus test fixtures.
pub mod support;

use hw_model_io::memory::{MemoryAccess, MemoryAddress, MemoryAlignment};
use pdma_model_dma::ChannelFatal;
use support::{TestResult, command, read_word, run, setup, submit};

#[test]
fn copy_retires_with_payload_visible_and_no_event_or_notification() -> TestResult {
    // Given
    let (mut device, memory) = setup(0x1100, 2)?;
    memory.write(
        MemoryAddress::new(0x1200),
        MemoryAlignment::ONE,
        &[3, 1, 4, 1],
    )?;
    submit(&mut device, &memory, command())?;
    // When
    run(&mut device)?;
    // Then
    let mut destination = [0; 4];
    memory.read(
        MemoryAddress::new(0x1300),
        MemoryAlignment::ONE,
        &mut destination,
    )?;
    assert_eq!(destination, [3, 1, 4, 1]);
    support::assert_no_event(&mut device)?;
    assert!(!device.irq());
    assert!(!device.take_interrupt());
    Ok(())
}

#[test]
fn event_write_fault_is_reported_without_event_memory() -> TestResult {
    // Given
    let (mut device, memory) = setup(0x8000, 2)?;
    let mut request = command();
    support::copy_mut(&mut request)?.source = CopyCommandSourceValues::Linear(LinearEndpoint {
        address: 0x9000,
        offset: 0,
    });
    submit(&mut device, &memory, request)?;
    // When
    let result = run(&mut device);
    // Then
    assert!(result.is_ok());
    assert!(device.irq());
    assert_eq!(
        read_word(
            &mut device,
            pdma_model_contract::dma::ChannelBlock::ALIGNMENT
                + pdma_model_contract::dma::ChannelBlock::CONTROL.offset()
                + ChannelRegisters::RECOVERY.offset()
                + ChannelRecovery::FATAL.offset()
        )?,
        ChannelFatal::EventWrite.code().raw()
    );
    assert_eq!(
        read_word(
            &mut device,
            pdma_model_contract::dma::ChannelBlock::ALIGNMENT
                + pdma_model_contract::dma::ChannelBlock::EVENT.offset()
                + EventRegisters::PRODUCER.offset()
        )?,
        0
    );
    device.write(
        pdma_model_contract::dma::ChannelBlock::ALIGNMENT
            + pdma_model_contract::dma::ChannelBlock::CONTROL.offset()
            + ChannelRegisters::IRQ.offset()
            + ChannelInterrupts::ENABLE.offset(),
        &0_u32.to_le_bytes(),
    )?;
    assert!(device.take_interrupt());
    assert!(!device.irq());
    run(&mut device)?;
    assert_eq!(
        read_word(
            &mut device,
            pdma_model_contract::dma::ChannelBlock::ALIGNMENT
                + pdma_model_contract::dma::ChannelBlock::COMMAND.offset()
                + CommandRegisters::CONSUMER.offset()
        )?,
        1
    );
    Ok(())
}

#[test]
fn source_access_failure_reports_a_stopping_event() -> TestResult {
    // Given
    let (mut device, memory) = setup(0x1100, 2)?;
    submit(&mut device, &memory, {
        let original = command();
        let pdma_model_contract::dma::Command::Copy(copy) = original else {
            return Err("copy fixture required".into());
        };
        Command::Copy(pdma_model_contract::dma::CopyCommand {
            id: original.id(),
            source: CopyCommandSourceValues::Linear(LinearEndpoint {
                address: 0x8000,
                offset: 0,
            }),
            ..copy
        })
    })?;
    // When
    run(&mut device)?;
    // Then
    let mut bytes = [0; Event::RECORD_SIZE];
    memory.read(MemoryAddress::new(0x1100), MemoryAlignment::ONE, &mut bytes)?;
    assert_eq!(Event::from_bytes(&bytes)?.code, EventCode::ReadError);
    assert!(device.irq());
    assert_eq!(
        read_word(
            &mut device,
            pdma_model_contract::dma::ChannelBlock::ALIGNMENT
                + pdma_model_contract::dma::ChannelBlock::CONTROL.offset()
                + ChannelRegisters::RECOVERY.offset()
                + ChannelRecovery::FATAL.offset()
        )?,
        0
    );
    Ok(())
}

#[test]
fn successive_successes_reuse_command_slots_without_event_consumption() -> TestResult {
    let (mut device, memory) = setup(0x1100, 1)?;
    submit(&mut device, &memory, command())?;
    run(&mut device)?;
    memory.write(
        MemoryAddress::new(0x1000),
        MemoryAlignment::ONE,
        &Command::Copy(pdma_model_contract::dma::CopyCommand {
            id: 8,
            ..*support::copy(&command())?
        })
        .to_bytes()?,
    )?;
    memory.write(MemoryAddress::new(0x1200), MemoryAlignment::ONE, &[2; 4])?;
    device.write(
        pdma_model_contract::dma::ChannelBlock::ALIGNMENT
            + pdma_model_contract::dma::ChannelBlock::COMMAND.offset()
            + CommandRegisters::PRODUCER.offset(),
        &2_u32.to_le_bytes(),
    )?;
    run(&mut device)?;
    assert_eq!(
        read_word(
            &mut device,
            pdma_model_contract::dma::ChannelBlock::ALIGNMENT
                + pdma_model_contract::dma::ChannelBlock::COMMAND.offset()
                + CommandRegisters::CONSUMER.offset()
        )?,
        2
    );
    assert_eq!(
        memory.read_buffer::<[u8; 4]>(MemoryAddress::new(0x1300), MemoryAlignment::ONE)?,
        [2; 4]
    );
    support::assert_no_event(&mut device)?;
    assert_eq!(
        read_word(
            &mut device,
            pdma_model_contract::dma::ChannelBlock::ALIGNMENT
                + pdma_model_contract::dma::ChannelBlock::CONTROL.offset()
                + ChannelRegisters::RECOVERY.offset()
                + ChannelRecovery::FATAL.offset()
        )?,
        0
    );
    Ok(())
}
