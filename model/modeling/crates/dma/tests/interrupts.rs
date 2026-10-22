//! Channel notification sources and independent acknowledgment.

/// Shared raw-bus fixtures.
pub mod support;

use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment};
use pdma_model_contract::dma::{
    ChannelIrqCauseValues, ChannelRegisters, Command, CommandRegisters, CopyCommand, Endpoint,
    EventRegisters,
};
use support::{TestResult, command, read_word, run, setup, submit};

const CONTROL: usize = pdma_model_contract::dma::ChannelBlock::ALIGNMENT
    + pdma_model_contract::dma::ChannelBlock::CONTROL.offset();
const COMMAND: usize = pdma_model_contract::dma::ChannelBlock::ALIGNMENT
    + pdma_model_contract::dma::ChannelBlock::COMMAND.offset();
const EVENT: usize = pdma_model_contract::dma::ChannelBlock::ALIGNMENT
    + pdma_model_contract::dma::ChannelBlock::EVENT.offset();

fn append(
    device: &mut pdma_model_dma::Dma<hardware_interface_runtime::SharedMemory>,
    memory: &hardware_interface_runtime::SharedMemory,
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
    Command::copy(
        7,
        CopyCommand {
            source: Endpoint::linear(0x1200, 0),
            destination: Endpoint::scatter_gather(0, 0),
            copy_length: 4,
            sg_context: 0,
        },
    )
}

#[test]
fn trap_obeys_only_its_own_mask() -> TestResult {
    for mask in 0..4_u32 {
        // Given: every combination of independent notification masks.
        let (mut device, memory) = setup(0x1800, 2)?;
        device.write(
            CONTROL + ChannelRegisters::IRQ_MASK.offset(),
            &mask.to_le_bytes(),
        )?;
        // When: a Trap executes without producing an error event.
        submit(&mut device, &memory, Command::trap(1))?;
        run(&mut device)?;
        // Then: only Trap's mask controls delivery; its cause remains visible.
        assert_eq!(device.take_interrupt(), mask & 2 == 0);
        let cause = ChannelIrqCauseValues::new(read_word(
            &mut device,
            CONTROL + ChannelRegisters::IRQ_CAUSE.offset(),
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
        memory.read_typed::<[u8; 4]>(MemoryAddress::new(0x1300), MemoryAlignment::ONE)?,
        *b"copy"
    );
    assert!(!device.take_interrupt());
    assert_eq!(
        read_word(&mut device, EVENT + EventRegisters::PRODUCER.offset())?,
        0
    );
    Ok(())
}

#[test]
fn repeated_traps_notify_and_unmask_does_not_replay() -> TestResult {
    // Given: a masked Trap whose sticky cause has not been acknowledged.
    let (mut device, memory) = setup(0x1800, 4)?;
    let mask = CONTROL + ChannelRegisters::IRQ_MASK.offset();
    device.write(mask, &2_u32.to_le_bytes())?;
    submit(&mut device, &memory, Command::trap(1))?;
    run(&mut device)?;
    assert!(!device.take_interrupt());
    // When: software unmasks, then submits two fresh Trap commands.
    device.write(mask, &0_u32.to_le_bytes())?;
    assert!(!device.take_interrupt());
    for index in 1..3 {
        append(&mut device, &memory, index, Command::trap(u64::from(index)))?;
        // Then: each new Trap can emit despite the already-set cause.
        assert!(device.take_interrupt());
        assert!(!device.take_interrupt());
    }
    Ok(())
}

#[test]
fn error_event_stops_channel_after_consumption_and_bypasses_routine_masks() -> TestResult {
    // Given: a bad command followed by a valid copy.
    let (mut device, memory) = setup(0x1800, 2)?;
    device.write(
        CONTROL + ChannelRegisters::IRQ_MASK.offset(),
        &3_u32.to_le_bytes(),
    )?;
    submit(&mut device, &memory, invalid())?;
    run(&mut device)?;
    assert!(device.take_interrupt());
    let cause = ChannelIrqCauseValues::new(read_word(
        &mut device,
        CONTROL + ChannelRegisters::IRQ_CAUSE.offset(),
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
        CONTROL + ChannelRegisters::IRQ_CAUSE.offset(),
    )?);
    assert!(cause.error());
    assert!(!cause.event_non_empty());
    Ok(())
}

#[test]
fn fatal_notifies_independently_of_sticky_trap() -> TestResult {
    // Given: an acknowledged notification whose Trap cause remains set.
    let (mut device, memory) = setup(0x1800, 2)?;
    submit(&mut device, &memory, Command::trap(1))?;
    run(&mut device)?;
    assert!(device.take_interrupt());
    let mut malformed = command().to_bytes()?;
    *malformed.get_mut(8).ok_or("missing kind")? = 255;
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
    // Then: fatal has its own notification and does not clear Trap.
    assert!(device.take_interrupt());
    let cause = ChannelIrqCauseValues::new(read_word(
        &mut device,
        CONTROL + ChannelRegisters::IRQ_CAUSE.offset(),
    )?);
    assert!(cause.trap() && cause.channel_fatal());
    Ok(())
}

#[test]
fn trap_ack_clears_cause_without_retracting_pending_notification() -> TestResult {
    // Given: a pending Trap notification.
    let (mut device, memory) = setup(0x1800, 2)?;
    submit(&mut device, &memory, Command::trap(1))?;
    run(&mut device)?;
    // When: software acknowledges the sticky source before platform delivery.
    let ack = CONTROL + ChannelRegisters::TRAP_ACK.offset();
    device.write(ack, &0_u32.to_le_bytes())?;
    assert!(
        ChannelIrqCauseValues::new(read_word(
            &mut device,
            CONTROL + ChannelRegisters::IRQ_CAUSE.offset()
        )?)
        .trap()
    );
    device.write(ack, &1_u32.to_le_bytes())?;
    // Then: the cause clears but the previously emitted notification survives.
    assert!(
        !ChannelIrqCauseValues::new(read_word(
            &mut device,
            CONTROL + ChannelRegisters::IRQ_CAUSE.offset()
        )?)
        .trap()
    );
    assert!(device.take_interrupt());
    assert!(!device.take_interrupt());
    Ok(())
}
