//! Adverse channel transactions and ring boundaries.

use pdma_model_contract::dma::{
    ChannelConfiguration, ChannelInterrupts, ChannelRecovery, ChannelRegisters, Command,
    CommandRegisters, CopyCommandDestinationValues, CopyCommandSourceValues, Event, EventCode,
    EventRegisters, LinearEndpoint,
};

/// Shared channel integration fixtures.
pub mod support;

use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment, SharedMemory};
use pdma_model_dma::{ChannelFatal, Dma, Error};
use support::{TestResult, command, read_word, run, setup, submit};

const COMMAND: usize = pdma_model_contract::dma::ChannelBlock::ALIGNMENT
    + pdma_model_contract::dma::ChannelBlock::COMMAND.offset();
const EVENT: usize = pdma_model_contract::dma::ChannelBlock::ALIGNMENT
    + pdma_model_contract::dma::ChannelBlock::EVENT.offset();
const CHANNEL: usize = pdma_model_contract::dma::ChannelBlock::ALIGNMENT
    + pdma_model_contract::dma::ChannelBlock::CONTROL.offset();
const ENABLE: usize =
    CHANNEL + ChannelRegisters::CONFIG.offset() + ChannelConfiguration::ENABLE.offset();
const IRQ_ENABLE: usize =
    CHANNEL + ChannelRegisters::IRQ.offset() + ChannelInterrupts::ENABLE.offset();
const FAULT: usize =
    CHANNEL + ChannelRegisters::RECOVERY.offset() + ChannelRecovery::FATAL.offset();
const PRODUCER: usize = COMMAND + CommandRegisters::PRODUCER.offset();
const CONSUMER: usize = EVENT + EventRegisters::CONSUMER.offset();
const COMMAND_CONSUMER: usize = COMMAND + CommandRegisters::CONSUMER.offset();
const EVENT_PRODUCER: usize = EVENT + EventRegisters::PRODUCER.offset();

fn disabled(base: u64, capacity: u32) -> Result<Dma<SharedMemory>, Box<dyn std::error::Error>> {
    let mut device = Dma::new(SharedMemory::new(), 1)?;
    device.write(
        COMMAND + CommandRegisters::BASE.offset(),
        &base.to_le_bytes(),
    )?;
    device.write(
        EVENT + EventRegisters::BASE.offset(),
        &0x1100_u64.to_le_bytes(),
    )?;
    device.write(
        COMMAND + CommandRegisters::CAPACITY.offset(),
        &capacity.to_le_bytes(),
    )?;
    device.write(
        EVENT + EventRegisters::CAPACITY.offset(),
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
        (EVENT_PRODUCER, 4, 1, Error::Access),
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
        let before = read_word(&mut device, observed)?;
        // When
        let bytes = u32::to_le_bytes(value);
        let result = device.write(offset, bytes.get(..width).ok_or("bad fixture width")?);
        // Then
        assert_eq!(result, Err(error), "offset {offset}");
        assert_eq!(read_word(&mut device, observed)?, before);
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
    let event = device.write(
        EVENT + EventRegisters::CAPACITY.offset(),
        &1_u32.to_le_bytes(),
    );
    // Then
    assert_eq!(command, Err(Error::ConfigurationLocked));
    assert_eq!(event, Err(Error::ConfigurationLocked));
    device.write(ENABLE, &1_u32.to_le_bytes())?;
    assert_eq!(
        read_word(&mut device, EVENT + EventRegisters::CAPACITY.offset())?,
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
        assert_eq!(read_word(&mut device, ENABLE)?, 0);
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
fn unmask_does_not_replay_command_read_fault() -> TestResult {
    // Given
    let mut device = disabled(0x8000, 2)?;
    device.write(ENABLE, &1_u32.to_le_bytes())?;
    device.write(PRODUCER, &1_u32.to_le_bytes())?;
    // When
    let result = run(&mut device);
    // Then
    assert!(result.is_ok());
    assert!(!device.irq());
    assert_eq!(
        read_word(&mut device, FAULT)?,
        ChannelFatal::CommandRead.code().raw()
    );
    assert_eq!(read_word(&mut device, COMMAND_CONSUMER)?, 0);
    assert_eq!(read_word(&mut device, EVENT_PRODUCER)?, 0);
    device.write(IRQ_ENABLE, &1_u32.to_le_bytes())?;
    assert!(!device.irq());
    Ok(())
}

#[test]
fn destination_fault_is_a_failed_completion() -> TestResult {
    // Given
    let (mut device, memory) = setup(0x1100, 2)?;
    submit(&mut device, &memory, {
        let original = command();
        let pdma_model_contract::dma::Command::Copy(copy) = original else {
            return Err("copy fixture required".into());
        };
        Command::Copy(pdma_model_contract::dma::CopyCommand {
            id: original.id(),
            destination: CopyCommandDestinationValues::Linear(LinearEndpoint {
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
    assert_eq!(Event::from_bytes(&bytes)?.code, EventCode::WriteError);
    assert!(device.irq());
    assert_eq!(read_word(&mut device, FAULT)?, 0);
    Ok(())
}

#[test]
fn zero_length_skips_payload_and_longer_copy_reports_inaccessible_source() -> TestResult {
    for (length, status) in [(0, None), (257, Some(EventCode::ReadError))] {
        // Given
        let (mut device, memory) = setup(0x1100, 2)?;
        submit(&mut device, &memory, {
            let original = command();
            let pdma_model_contract::dma::Command::Copy(copy) = original else {
                return Err("copy fixture required".into());
            };
            Command::Copy(pdma_model_contract::dma::CopyCommand {
                id: original.id(),
                copy_length: length,
                source: CopyCommandSourceValues::Linear(LinearEndpoint {
                    address: 0x8000,
                    offset: 0,
                }),
                destination: CopyCommandDestinationValues::Linear(LinearEndpoint {
                    address: 0x9000,
                    offset: 0,
                }),
                ..copy
            })
        })?;
        // When
        run(&mut device)?;
        // Then
        if let Some(code) = status {
            let bytes = memory.read_typed::<pdma_model_contract::dma::EventRaw>(
                0x1100.into(),
                MemoryAlignment::ONE,
            )?;
            let event = Event::from_bytes(bytes.as_ref())?;
            assert_eq!(
                (event.id, event.code, event.bytes_completed()),
                (7, code, 0)
            );
            assert!(device.irq());
        } else {
            support::assert_no_event(&mut device)?;
            assert!(!device.irq());
        }
    }
    Ok(())
}

#[test]
fn rings_reuse_slots_in_order_after_wrap() -> TestResult {
    // Given
    let (mut device, memory) = setup(0x1100, 2)?;
    let mut copied = Vec::new();
    // When
    for cursor in 0_u32..5 {
        let slot = u64::from(cursor % 2);
        memory.write(
            MemoryAddress::new(0x1000 + slot * u64::try_from(Command::RECORD_SIZE)?),
            MemoryAlignment::ONE,
            &Command::Copy(pdma_model_contract::dma::CopyCommand {
                id: u64::from(cursor),
                ..*support::copy(&command())?
            })
            .to_bytes()?,
        )?;
        memory.write(
            0x1200.into(),
            MemoryAlignment::ONE,
            &[u8::try_from(cursor)?; 4],
        )?;
        device.write(PRODUCER, &(cursor + 1).to_le_bytes())?;
        run(&mut device)?;
        copied.push(memory.read_typed::<[u8; 4]>(0x1300.into(), MemoryAlignment::ONE)?);
        assert!(!device.take_interrupt());
    }
    // Then
    assert_eq!(copied, [[0; 4], [1; 4], [2; 4], [3; 4], [4; 4]]);
    assert!(!device.irq());
    assert_eq!(read_word(&mut device, COMMAND_CONSUMER)?, 5);
    assert_eq!(read_word(&mut device, EVENT_PRODUCER)?, 0);
    Ok(())
}
