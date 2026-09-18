//! Raw bus, copy publication and channel-fault behavior.

/// Shared raw-bus test fixtures.
pub mod support;

use hardware_interface_pipeline::{Operation, RunOutcome, StepExecutor};
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment, SharedMemory};
use pdma_model_contract::{
    ChannelRegisters, Command, CommandRegisters, Endpoint, Response, ResponseRegisters,
    ResponseStatus,
};
use pdma_model_dma::{Dma, Fault, Pipeline};
use support::{TestResult, command, read_word, run, setup, submit};

#[test]
fn copy_publishes_response_before_notification_is_taken() -> TestResult {
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
    let mut bytes = [0; Response::RECORD_SIZE];
    memory.read(MemoryAddress::new(0x1100), MemoryAlignment::ONE, &mut bytes)?;
    assert_eq!(
        Response::from_bytes(&bytes)?,
        Response {
            flags: pdma_model_contract::CompletionFlagsValues::default(),
            id: 7,
            status: ResponseStatus::Success,
            bytes_completed: 4,
        }
    );
    assert!(device.irq());
    assert!(device.take_interrupt());
    device.write(
        pdma_model_contract::ChannelBlock::ALIGNMENT
            + pdma_model_contract::ChannelBlock::RESPONSE.offset()
            + ResponseRegisters::CONSUMER.offset(),
        &1_u32.to_le_bytes(),
    )?;
    assert!(!device.irq());
    Ok(())
}

#[test]
fn response_write_fault_is_reported_without_response_memory() -> TestResult {
    // Given
    let (mut device, memory) = setup(0x8000, 2)?;
    submit(&mut device, &memory, command())?;
    // When
    let result = run(&mut device);
    // Then
    assert!(result.is_err());
    assert!(device.irq());
    assert_eq!(
        read_word(
            &mut device,
            pdma_model_contract::ChannelBlock::ALIGNMENT
                + pdma_model_contract::ChannelBlock::CONTROL.offset()
                + ChannelRegisters::FAULT.offset()
        )?,
        Fault::ResponseWrite.code().raw()
    );
    assert_eq!(
        read_word(
            &mut device,
            pdma_model_contract::ChannelBlock::ALIGNMENT
                + pdma_model_contract::ChannelBlock::RESPONSE.offset()
                + ResponseRegisters::PRODUCER.offset()
        )?,
        0
    );
    device.write(
        pdma_model_contract::ChannelBlock::ALIGNMENT
            + pdma_model_contract::ChannelBlock::CONTROL.offset()
            + ChannelRegisters::IRQ_ENABLE.offset(),
        &0_u32.to_le_bytes(),
    )?;
    assert!(device.take_interrupt());
    assert!(!device.irq());
    assert!(run(&mut device).is_err());
    assert_eq!(
        read_word(
            &mut device,
            pdma_model_contract::ChannelBlock::ALIGNMENT
                + pdma_model_contract::ChannelBlock::COMMAND.offset()
                + CommandRegisters::CONSUMER.offset()
        )?,
        1
    );
    Ok(())
}

#[test]
fn source_fault_produces_failure_response_not_success() -> TestResult {
    // Given
    let (mut device, memory) = setup(0x1100, 2)?;
    submit(
        &mut device,
        &memory,
        Command {
            source: Endpoint::linear(0x8000, 4),
            ..command()
        },
    )?;
    // When
    run(&mut device)?;
    // Then
    let mut bytes = [0; Response::RECORD_SIZE];
    memory.read(MemoryAddress::new(0x1100), MemoryAlignment::ONE, &mut bytes)?;
    assert_eq!(
        Response::from_bytes(&bytes)?.status,
        ResponseStatus::ReadFault
    );
    assert!(device.irq());
    assert_eq!(
        read_word(
            &mut device,
            pdma_model_contract::ChannelBlock::ALIGNMENT
                + pdma_model_contract::ChannelBlock::CONTROL.offset()
                + ChannelRegisters::FAULT.offset()
        )?,
        0
    );
    Ok(())
}

#[test]
fn full_response_ring_defers_command_acceptance_until_space_is_available() -> TestResult {
    // Given
    let (mut device, memory) = setup(0x1100, 1)?;
    submit(&mut device, &memory, command())?;
    run(&mut device)?;
    memory.write(
        MemoryAddress::new(0x1000),
        MemoryAlignment::ONE,
        &Command { id: 8, ..command() }.to_bytes()?,
    )?;
    memory.write(MemoryAddress::new(0x1200), MemoryAlignment::ONE, &[2; 4])?;
    device.write(
        pdma_model_contract::ChannelBlock::ALIGNMENT
            + pdma_model_contract::ChannelBlock::COMMAND.offset()
            + CommandRegisters::PRODUCER.offset(),
        &2_u32.to_le_bytes(),
    )?;
    let mut hardware = support::single_command(&device);
    let mut operation = Operation::<Pipeline, Dma<SharedMemory>>::new(());
    // When
    let mut stalled = false;
    for _ in 0..32 {
        if matches!(
            StepExecutor::new()
                .step(&mut hardware, &mut operation, &mut device)
                .map_err(|e| format!("{e:?}"))?,
            RunOutcome::Pending(())
        ) {
            stalled = true;
            break;
        }
    }
    assert_eq!(
        read_word(
            &mut device,
            pdma_model_contract::ChannelBlock::ALIGNMENT
                + pdma_model_contract::ChannelBlock::COMMAND.offset()
                + CommandRegisters::CONSUMER.offset()
        )?,
        1
    );
    assert_eq!(
        memory.read_typed::<[u8; 4]>(MemoryAddress::new(0x1300), MemoryAlignment::ONE)?,
        [0; 4]
    );
    device.write(
        pdma_model_contract::ChannelBlock::ALIGNMENT
            + pdma_model_contract::ChannelBlock::RESPONSE.offset()
            + ResponseRegisters::CONSUMER.offset(),
        &1_u32.to_le_bytes(),
    )?;
    let mut finished = false;
    for _ in 0..32 {
        match StepExecutor::new()
            .step(&mut hardware, &mut operation, &mut device)
            .map_err(|e| format!("{e:?}"))?
        {
            RunOutcome::Complete(result) => {
                result?;
                finished = true;
                break;
            }
            RunOutcome::Progressed => {}
            RunOutcome::Pending(()) => return Err("space was released but command stalled".into()),
        }
    }
    // Then
    assert!(stalled);
    assert!(finished);
    let mut destination = [0; 4];
    memory.read(
        MemoryAddress::new(0x1300),
        MemoryAlignment::ONE,
        &mut destination,
    )?;
    assert_eq!(destination, [2; 4]);
    assert_eq!(
        read_word(
            &mut device,
            pdma_model_contract::ChannelBlock::ALIGNMENT
                + pdma_model_contract::ChannelBlock::CONTROL.offset()
                + ChannelRegisters::FAULT.offset()
        )?,
        0
    );
    Ok(())
}
