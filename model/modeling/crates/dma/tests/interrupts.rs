//! Channel notification sources and independent acknowledgment.

use pdma_model_contract::dma::{
    ChannelInterrupts, ChannelIrqCauseValues, ChannelRegisters, Command, CommandHeader,
    CommandRegisters, CopyCommand, CopyCommandDestinationValues, CopyCommandSourceValues,
    EventRegisters, LinearEndpoint, Opcode, ScatterGatherEndpoint, TrapCommand,
};

/// Shared raw-bus fixtures.
pub mod support;

use hw_model_io::memory::{MemoryAccess, MemoryAddress, MemoryAlignment};
use support::{TestResult, command, read_word, run, setup, submit};

const CONTROL: usize = pdma_model_contract::dma::ChannelBlock::ALIGNMENT
    + pdma_model_contract::dma::ChannelBlock::CONTROL.offset();
const COMMAND: usize = pdma_model_contract::dma::ChannelBlock::ALIGNMENT
    + pdma_model_contract::dma::ChannelBlock::COMMAND.offset();
const EVENT: usize = pdma_model_contract::dma::ChannelBlock::ALIGNMENT
    + pdma_model_contract::dma::ChannelBlock::EVENT.offset();

fn append(
    device: &mut pdma_model_dma::Dma<hw_model_io::memory::SharedMemory>,
    memory: &hw_model_io::memory::SharedMemory,
    index: u32,
    input: Command,
) -> TestResult {
    memory.write(
        MemoryAddress::new(0x1000 + u64::from(index) * u64::try_from(Command::RECORD_SIZE)?),
        MemoryAlignment::ONE,
        &input.to_bytes()?,
    )?;
    device.write(
        COMMAND + CommandRegisters::PRODUCER.offset(),
        &(index + 1).to_le_bytes(),
    )?;
    run(device)
}

const fn invalid() -> Command {
    Command::Copy(CopyCommand {
        id: 7,
        source: CopyCommandSourceValues::Linear(LinearEndpoint {
            address: 0x1200,
            offset: 0,
        }),
        destination: CopyCommandDestinationValues::ScatterGather(ScatterGatherEndpoint {
            list_address: 0,
            count: 0,
            index: 0,
            offset: 0,
        }),
        copy_length: 4,
        sg_context: 0,
        ..pdma_model_contract::dma::CopyCommand::ZERO
    })
}

#[test]
fn trap_obeys_only_its_own_mask() -> TestResult {
    for mask in 0..4_u32 {
        // Given: every combination of independent notification masks.
        let (mut device, memory) = setup(0x1800, 2)?;
        device.write(
            CONTROL + ChannelRegisters::IRQ.offset() + ChannelInterrupts::MASK.offset(),
            &mask.to_le_bytes(),
        )?;
        // When: a Trap executes without producing an error event.
        submit(
            &mut device,
            &memory,
            Command::Trap(TrapCommand {
                header: CommandHeader {
                    opcode: Opcode::Trap,
                },
                id: 1,
            }),
        )?;
        run(&mut device)?;
        // Then: only Trap's mask controls delivery; its cause remains visible.
        assert_eq!(device.irq(), mask & 2 == 0);
        let cause = ChannelIrqCauseValues::new(read_word(
            &mut device,
            CONTROL + ChannelRegisters::IRQ.offset() + ChannelInterrupts::CAUSE.offset(),
        )?);
        assert!(cause.trap());
        assert!(!cause.event_non_empty());
    }
    Ok(())
}

#[test]
fn successful_copy_is_silent() -> TestResult {
    // Given: a normal copy with both routine sources enabled.
    let (mut device, memory) = setup(0x1800, 2)?;
    memory.write(MemoryAddress::new(0x1200), MemoryAlignment::ONE, b"copy")?;
    // When: the copy completes.
    submit(&mut device, &memory, command())?;
    run(&mut device)?;
    // Then: payload changes, but no success event or implicit interrupt exists.
    assert_eq!(
        memory.read_buffer::<[u8; 4]>(MemoryAddress::new(0x1300), MemoryAlignment::ONE)?,
        *b"copy"
    );
    assert!(!device.irq());
    assert_eq!(
        read_word(&mut device, EVENT + EventRegisters::PRODUCER.offset())?,
        0
    );
    Ok(())
}

#[test]
fn unmask_asserts_a_sticky_trap_and_repeated_traps_keep_it_high() -> TestResult {
    // Given: a masked Trap whose sticky cause has not been acknowledged.
    let (mut device, memory) = setup(0x1800, 4)?;
    let mask = CONTROL + ChannelRegisters::IRQ.offset() + ChannelInterrupts::MASK.offset();
    device.write(mask, &2_u32.to_le_bytes())?;
    submit(
        &mut device,
        &memory,
        Command::Trap(TrapCommand {
            header: CommandHeader {
                opcode: Opcode::Trap,
            },
            id: 1,
        }),
    )?;
    run(&mut device)?;
    assert!(!device.irq());
    // When: software unmasks, then submits two fresh Trap commands.
    device.write(mask, &0_u32.to_le_bytes())?;
    assert!(device.irq());
    for index in 1..3 {
        append(
            &mut device,
            &memory,
            index,
            Command::Trap(TrapCommand {
                header: CommandHeader {
                    opcode: Opcode::Trap,
                },
                id: u64::from(index),
            }),
        )?;
        // The sticky source remains high across repeated Traps.
        assert!(device.irq());
    }
    Ok(())
}

#[test]
fn error_event_stops_channel_after_consumption_and_bypasses_routine_masks() -> TestResult {
    // Given: a bad command followed by a valid copy.
    let (mut device, memory) = setup(0x1800, 2)?;
    device.write(
        CONTROL + ChannelRegisters::IRQ.offset() + ChannelInterrupts::MASK.offset(),
        &3_u32.to_le_bytes(),
    )?;
    submit(&mut device, &memory, invalid())?;
    run(&mut device)?;
    assert!(device.irq());
    let cause = ChannelIrqCauseValues::new(read_word(
        &mut device,
        CONTROL + ChannelRegisters::IRQ.offset() + ChannelInterrupts::CAUSE.offset(),
    )?);
    assert!(cause.event_non_empty() && cause.error());
    // When: software consumes the error and publishes more work without reset.
    device.write(
        EVENT + EventRegisters::CONSUMER.offset(),
        &1_u32.to_le_bytes(),
    )?;
    let _outcome = append(&mut device, &memory, 1, command());
    // Then: consumption leaves the stop latched and the successor unfetched.
    assert_eq!(
        read_word(&mut device, COMMAND + CommandRegisters::CONSUMER.offset())?,
        1
    );
    let cause = ChannelIrqCauseValues::new(read_word(
        &mut device,
        CONTROL + ChannelRegisters::IRQ.offset() + ChannelInterrupts::CAUSE.offset(),
    )?);
    assert!(cause.error());
    assert!(!cause.event_non_empty());
    Ok(())
}

#[test]
fn channel_fatal_keeps_irq_asserted_after_trap_ack() -> TestResult {
    // Given: a Trap with its IRQ line asserted.
    let (mut device, memory) = setup(0x1800, 2)?;
    submit(
        &mut device,
        &memory,
        Command::Trap(TrapCommand {
            header: CommandHeader {
                opcode: Opcode::Trap,
            },
            id: 1,
        }),
    )?;
    run(&mut device)?;
    assert!(device.irq());
    let mut malformed = command().to_bytes()?;
    *malformed.get_mut(0).ok_or("missing opcode")? = 255;
    memory.write(
        MemoryAddress::new(0x1000 + u64::try_from(Command::RECORD_SIZE)?),
        MemoryAlignment::ONE,
        &malformed,
    )?;
    // When: the next descriptor cannot decode.
    device.write(
        COMMAND + CommandRegisters::PRODUCER.offset(),
        &2_u32.to_le_bytes(),
    )?;
    run(&mut device)?;
    // Then: both causes share the channel line.
    assert!(device.irq());
    let cause = ChannelIrqCauseValues::new(read_word(
        &mut device,
        CONTROL + ChannelRegisters::IRQ.offset() + ChannelInterrupts::CAUSE.offset(),
    )?);
    assert!(cause.trap() && cause.channel_fatal());
    device.write(
        CONTROL + ChannelRegisters::IRQ.offset() + ChannelInterrupts::TRAP_ACK.offset(),
        &1_u32.to_le_bytes(),
    )?;
    assert!(device.irq());
    Ok(())
}

#[test]
fn trap_ack_deasserts_the_line() -> TestResult {
    // Given: a pending Trap notification.
    let (mut device, memory) = setup(0x1800, 2)?;
    submit(
        &mut device,
        &memory,
        Command::Trap(TrapCommand {
            header: CommandHeader {
                opcode: Opcode::Trap,
            },
            id: 1,
        }),
    )?;
    run(&mut device)?;
    // When: software acknowledges the sticky source with the line asserted.
    let ack = CONTROL + ChannelRegisters::IRQ.offset() + ChannelInterrupts::TRAP_ACK.offset();
    device.write(ack, &0_u32.to_le_bytes())?;
    assert!(
        ChannelIrqCauseValues::new(read_word(
            &mut device,
            CONTROL + ChannelRegisters::IRQ.offset() + ChannelInterrupts::CAUSE.offset()
        )?)
        .trap()
    );
    device.write(ack, &1_u32.to_le_bytes())?;
    // Then: the cause and the output level both clear.
    assert!(
        !ChannelIrqCauseValues::new(read_word(
            &mut device,
            CONTROL + ChannelRegisters::IRQ.offset() + ChannelInterrupts::CAUSE.offset()
        )?)
        .trap()
    );
    assert!(!device.irq());
    Ok(())
}
