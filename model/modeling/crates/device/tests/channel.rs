//! Raw bus, copy publication and channel-fault behavior.

/// Shared raw-bus test fixtures.
pub mod support;

use hardware_interface_pipeline::{Operation, RunOutcome, StepExecutor};
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment, SharedMemory};
use pdma_model_contract::{
    ChannelRegisters, Command, CommandRegisters, DeviceRegisters, Response, ResponseRegisters,
    ResponseStatus,
};
use pdma_model_device::{Device, Fault, Pipeline, pipeline};
use support::{TestResult, command, read_word, run, setup, submit};

#[test]
fn copy_publishes_response_before_irq_and_drain_lowers_line() -> TestResult {
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
            id: 7,
            status: ResponseStatus::Success
        }
    );
    assert!(device.irq());
    device.write(
        DeviceRegisters::RESPONSE.offset() + ResponseRegisters::CONSUMER.offset(),
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
    assert_eq!(
        read_word(
            &device,
            DeviceRegisters::CHANNEL.offset() + ChannelRegisters::FAULT.offset()
        )?,
        Fault::ResponseWrite.code().raw()
    );
    assert_eq!(
        read_word(
            &device,
            DeviceRegisters::RESPONSE.offset() + ResponseRegisters::PRODUCER.offset()
        )?,
        0
    );
    assert!(device.irq());
    device.write(
        DeviceRegisters::CHANNEL.offset() + ChannelRegisters::IRQ_ENABLE.offset(),
        &0_u32.to_le_bytes(),
    )?;
    assert!(!device.irq());
    assert!(run(&mut device).is_err());
    assert_eq!(
        read_word(
            &device,
            DeviceRegisters::COMMAND.offset() + CommandRegisters::CONSUMER.offset()
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
            source: 0x8000,
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
    assert_eq!(
        read_word(
            &device,
            DeviceRegisters::CHANNEL.offset() + ChannelRegisters::FAULT.offset()
        )?,
        0
    );
    assert!(device.irq());
    Ok(())
}

#[test]
fn full_response_ring_stalls_retained_publication_without_replaying_copy() -> TestResult {
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
        DeviceRegisters::COMMAND.offset() + CommandRegisters::PRODUCER.offset(),
        &2_u32.to_le_bytes(),
    )?;
    let mut hardware = pipeline();
    let mut operation = Operation::<Pipeline, Device<SharedMemory>>::new(());
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
    memory.write(MemoryAddress::new(0x1300), MemoryAlignment::ONE, &[9; 4])?;
    device.write(
        DeviceRegisters::RESPONSE.offset() + ResponseRegisters::CONSUMER.offset(),
        &1_u32.to_le_bytes(),
    )?;
    let result = StepExecutor::new()
        .step(&mut hardware, &mut operation, &mut device)
        .map_err(|e| format!("{e:?}"))?;
    // Then
    assert!(stalled);
    assert!(matches!(result, RunOutcome::Complete(Ok(()))));
    let mut destination = [0; 4];
    memory.read(
        MemoryAddress::new(0x1300),
        MemoryAlignment::ONE,
        &mut destination,
    )?;
    assert_eq!(destination, [9; 4]);
    assert_eq!(
        read_word(
            &device,
            DeviceRegisters::CHANNEL.offset() + ChannelRegisters::FAULT.offset()
        )?,
        0
    );
    Ok(())
}
