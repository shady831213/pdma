//! Adverse channel transactions and ring boundaries.

/// Shared channel integration fixtures.
pub mod support;

use hardware_interface_pipeline::{Operation, RunOutcome, StepExecutor};
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment, SharedMemory};
use pdma_model_contract::{
    ChannelRegisters, Command, CommandRegisters, DeviceRegisters, Response, ResponseRegisters,
    ResponseStatus,
};
use pdma_model_device::{Device, Error, Fault, Pipeline, pipeline};
use support::{TestResult, command, read_word, run, setup, submit};

const COMMAND: usize = DeviceRegisters::COMMAND.offset();
const RESPONSE: usize = DeviceRegisters::RESPONSE.offset();
const CHANNEL: usize = DeviceRegisters::CHANNEL.offset();
const ENABLE: usize = CHANNEL + ChannelRegisters::ENABLE.offset();
const IRQ_ENABLE: usize = CHANNEL + ChannelRegisters::IRQ_ENABLE.offset();
const FAULT: usize = CHANNEL + ChannelRegisters::FAULT.offset();
const PRODUCER: usize = COMMAND + CommandRegisters::PRODUCER.offset();
const CONSUMER: usize = RESPONSE + ResponseRegisters::CONSUMER.offset();
const COMMAND_CONSUMER: usize = COMMAND + CommandRegisters::CONSUMER.offset();
const RESPONSE_PRODUCER: usize = RESPONSE + ResponseRegisters::PRODUCER.offset();

fn disabled(base: u64, capacity: u32) -> Result<Device<SharedMemory>, Box<dyn std::error::Error>> {
    let mut device = Device::new(SharedMemory::new());
    device.write(
        COMMAND + CommandRegisters::BASE.offset(),
        &base.to_le_bytes(),
    )?;
    device.write(
        RESPONSE + ResponseRegisters::BASE.offset(),
        &0x1100_u64.to_le_bytes(),
    )?;
    device.write(
        COMMAND + CommandRegisters::CAPACITY.offset(),
        &capacity.to_le_bytes(),
    )?;
    device.write(
        RESPONSE + ResponseRegisters::CAPACITY.offset(),
        &2_u32.to_le_bytes(),
    )?;
    Ok(device)
}

#[test]
fn malformed_writes_preserve_registers() -> TestResult {
    // Given
    let (mut device, _) = setup(0x1100, 2)?;
    let cases = [
        (COMMAND_CONSUMER, 4, 1, Error::Access),
        (RESPONSE_PRODUCER, 4, 1, Error::Access),
        (FAULT, 4, 1, Error::Access),
        (PRODUCER, 1, 1, Error::Width),
        (ENABLE, 4, 2, Error::ReservedBits),
        (IRQ_ENABLE, 4, 2, Error::ReservedBits),
        (PRODUCER + 1, 4, 1, Error::Unmapped),
        (PRODUCER, 4, 3, Error::Cursor),
        (CONSUMER, 4, 1, Error::Cursor),
    ];
    for (offset, width, value, error) in cases {
        let observed = if offset == PRODUCER + 1 {
            PRODUCER
        } else {
            offset
        };
        let before = read_word(&device, observed)?;
        // When
        let bytes = u32::to_le_bytes(value);
        let result = device.write(offset, bytes.get(..width).ok_or("bad fixture width")?);
        // Then
        assert_eq!(result, Err(error), "offset {offset}");
        assert_eq!(read_word(&device, observed)?, before);
        assert!(!device.irq());
    }
    Ok(())
}

#[test]
fn geometry_remains_locked_after_disable() -> TestResult {
    // Given
    let (mut device, _) = setup(0x1100, 2)?;
    device.write(ENABLE, &0_u32.to_le_bytes())?;
    // When
    let command = device.write(
        COMMAND + CommandRegisters::BASE.offset(),
        &0x1400_u64.to_le_bytes(),
    );
    let response = device.write(
        RESPONSE + ResponseRegisters::CAPACITY.offset(),
        &1_u32.to_le_bytes(),
    );
    // Then
    assert_eq!(command, Err(Error::ConfigurationLocked));
    assert_eq!(response, Err(Error::ConfigurationLocked));
    device.write(ENABLE, &1_u32.to_le_bytes())?;
    assert_eq!(
        read_word(&device, RESPONSE + ResponseRegisters::CAPACITY.offset())?,
        2
    );
    Ok(())
}

#[test]
fn invalid_geometry_rejects_enable() -> TestResult {
    for (base, capacity) in [
        (0x1000, 0),
        (0x1000, 3),
        (0x1001, 2),
        (u64::MAX - 7, 2),
        (0x1000, u32::MAX),
    ] {
        // Given
        let mut device = disabled(base, capacity)?;
        // When
        let result = device.write(ENABLE, &1_u32.to_le_bytes());
        // Then
        assert_eq!(result, Err(Error::Configuration));
        assert_eq!(read_word(&device, ENABLE)?, 0);
    }
    Ok(())
}

#[test]
fn pending_commands_cannot_exceed_capacity_when_enabled() -> TestResult {
    // Given
    let mut device = disabled(0x1000, 2)?;
    device.write(PRODUCER, &2_u32.to_le_bytes())?;
    // When
    let shrink = device.write(
        COMMAND + CommandRegisters::CAPACITY.offset(),
        &1_u32.to_le_bytes(),
    );
    let enable = device.write(ENABLE, &1_u32.to_le_bytes());
    // Then
    assert!(
        shrink.is_err() || enable.is_err(),
        "capacity shrink and enable both accepted two pending commands in one slot"
    );
    Ok(())
}

#[test]
fn command_read_fault_stays_masked_until_unmasked() -> TestResult {
    // Given
    let mut device = disabled(0x8000, 2)?;
    device.write(ENABLE, &1_u32.to_le_bytes())?;
    device.write(PRODUCER, &1_u32.to_le_bytes())?;
    // When
    let result = run(&mut device);
    // Then
    assert!(result.is_err());
    assert_eq!(read_word(&device, FAULT)?, Fault::CommandRead.code().raw());
    assert_eq!(read_word(&device, COMMAND_CONSUMER)?, 0);
    assert_eq!(read_word(&device, RESPONSE_PRODUCER)?, 0);
    assert!(!device.irq());
    device.write(IRQ_ENABLE, &1_u32.to_le_bytes())?;
    assert!(device.irq());
    Ok(())
}

#[test]
fn destination_fault_is_a_failed_completion() -> TestResult {
    // Given
    let (mut device, memory) = setup(0x1100, 2)?;
    submit(
        &mut device,
        &memory,
        Command {
            destination: 0x8000,
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
        ResponseStatus::WriteFault
    );
    assert_eq!(read_word(&device, FAULT)?, 0);
    assert!(device.irq());
    Ok(())
}

#[test]
fn length_boundaries_skip_inaccessible_payload_memory() -> TestResult {
    for (length, status) in [
        (0, ResponseStatus::Success),
        (257, ResponseStatus::InvalidLength),
    ] {
        // Given
        let (mut device, memory) = setup(0x1100, 2)?;
        submit(
            &mut device,
            &memory,
            Command {
                source: u64::MAX,
                destination: u64::MAX,
                length,
                ..command()
            },
        )?;
        // When
        run(&mut device)?;
        // Then
        let mut bytes = [0; Response::RECORD_SIZE];
        memory.read(MemoryAddress::new(0x1100), MemoryAlignment::ONE, &mut bytes)?;
        assert_eq!(Response::from_bytes(&bytes)?, Response { id: 7, status });
        assert!(device.irq());
    }
    Ok(())
}

#[test]
fn rings_reuse_slots_in_order_after_wrap() -> TestResult {
    // Given
    let (mut device, memory) = setup(0x1100, 2)?;
    let mut ids = Vec::new();
    // When
    for cursor in 0_u32..5 {
        let slot = u64::from(cursor % 2);
        memory.write(
            MemoryAddress::new(0x1000 + slot * u64::try_from(Command::RECORD_SIZE)?),
            MemoryAlignment::ONE,
            &Command {
                id: u64::from(cursor),
                ..command()
            }
            .to_bytes()?,
        )?;
        device.write(PRODUCER, &(cursor + 1).to_le_bytes())?;
        run(&mut device)?;
        let mut bytes = [0; Response::RECORD_SIZE];
        memory.read(
            MemoryAddress::new(0x1100 + slot * u64::try_from(Response::RECORD_SIZE)?),
            MemoryAlignment::ONE,
            &mut bytes,
        )?;
        ids.push(Response::from_bytes(&bytes)?.id);
        device.write(CONSUMER, &(cursor + 1).to_le_bytes())?;
    }
    // Then
    assert_eq!(ids, [0, 1, 2, 3, 4]);
    assert_eq!(read_word(&device, COMMAND_CONSUMER)?, 5);
    assert_eq!(read_word(&device, RESPONSE_PRODUCER)?, 5);
    assert!(!device.irq());
    Ok(())
}

#[test]
fn irq_requires_published_response_on_every_step() -> TestResult {
    // Given
    let (mut device, memory) = setup(0x1100, 2)?;
    submit(&mut device, &memory, command())?;
    let mut hardware = pipeline();
    let mut operation = Operation::<Pipeline, Device<SharedMemory>>::new(());
    // When
    for _ in 0..32 {
        let outcome = StepExecutor::new()
            .step(&mut hardware, &mut operation, &mut device)
            .map_err(|error| format!("{error:?}"))?;
        // Then
        let published = read_word(&device, RESPONSE_PRODUCER)?;
        assert_eq!(device.irq(), published == 1);
        if device.irq() {
            let mut bytes = [0; Response::RECORD_SIZE];
            memory.read(MemoryAddress::new(0x1100), MemoryAlignment::ONE, &mut bytes)?;
            assert_eq!(
                Response::from_bytes(&bytes)?,
                Response {
                    id: 7,
                    status: ResponseStatus::Success
                }
            );
        }
        match outcome {
            RunOutcome::Progressed => {}
            RunOutcome::Pending(()) => return Err("unexpected stall".into()),
            RunOutcome::Complete(result) => {
                result?;
                assert!(device.irq());
                return Ok(());
            }
        }
    }
    Err("pipeline exceeded step bound".into())
}
