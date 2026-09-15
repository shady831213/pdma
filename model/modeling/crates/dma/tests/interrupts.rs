//! Response notification conditions observed through memory and MMIO.

/// Shared raw-bus fixtures.
pub mod support;

use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment};
use pdma_model_contract::{ChannelRegisters, CommandRegisters, DeviceRegisters, ResponseRegisters};
use support::{TestResult, command, read_word, run, setup, submit};

const CONTROL: usize =
    DeviceRegisters::CHANNELS_OFFSET + pdma_model_contract::ChannelBlock::CONTROL.offset();
const COMMAND: usize =
    DeviceRegisters::CHANNELS_OFFSET + pdma_model_contract::ChannelBlock::COMMAND.offset();
const RESPONSE: usize =
    DeviceRegisters::CHANNELS_OFFSET + pdma_model_contract::ChannelBlock::RESPONSE.offset();

#[test]
fn each_response_source_obeys_only_its_own_mask() -> TestResult {
    for mask in 0..4_u32 {
        // Given: every combination of independent masks and a delivered first response.
        let (mut device, memory) = setup(0x1100, 2)?;
        device.write(
            CONTROL + ChannelRegisters::IRQ_MASK.offset(),
            &mask.to_le_bytes(),
        )?;
        submit(&mut device, &memory, command())?;
        run(&mut device)?;
        assert_eq!(device.take_interrupt(), mask & 1 == 0);
        let mut ioc = command();
        ioc.flags.ioc = true;
        memory.write(
            MemoryAddress::new(0x1040),
            MemoryAlignment::ONE,
            &ioc.to_bytes()?,
        )?;
        device.write(
            COMMAND + CommandRegisters::PRODUCER.offset(),
            &2_u32.to_le_bytes(),
        )?;
        // When: an IOC arrives without a new empty-to-non-empty transition.
        run(&mut device)?;
        // Then: only the IOC mask decides delivery; causes ignore both masks.
        assert_eq!(device.take_interrupt(), mask & 2 == 0);
        assert_eq!(
            read_word(&mut device, CONTROL + ChannelRegisters::IRQ_CAUSE.offset())?,
            3
        );
    }
    Ok(())
}

#[test]
fn non_empty_notifies_only_on_transition() -> TestResult {
    // Given: a delivered notification for the first response.
    let (mut device, memory) = setup(0x1100, 2)?;
    submit(&mut device, &memory, command())?;
    run(&mut device)?;
    assert!(device.take_interrupt());
    assert!(!device.irq());
    memory.write(
        MemoryAddress::new(0x1040),
        MemoryAlignment::ONE,
        &command().to_bytes()?,
    )?;
    device.write(
        COMMAND + CommandRegisters::PRODUCER.offset(),
        &2_u32.to_le_bytes(),
    )?;
    // When: a second response arrives while the first remains unread.
    run(&mut device)?;
    // Then: the cause remains visible without a duplicate notification.
    assert!(!device.take_interrupt());
    assert_eq!(
        read_word(&mut device, CONTROL + ChannelRegisters::IRQ_CAUSE.offset())?,
        1
    );
    device.write(
        RESPONSE + ResponseRegisters::CONSUMER.offset(),
        &1_u32.to_le_bytes(),
    )?;
    assert!(!device.irq());
    Ok(())
}

#[test]
fn ioc_notifies_while_non_empty_and_mask_does_not_replay() -> TestResult {
    // Given: queue notifications masked but IOC enabled.
    let (mut device, memory) = setup(0x1100, 2)?;
    let mask = CONTROL + ChannelRegisters::IRQ_MASK.offset();
    device.write(mask, &1_u32.to_le_bytes())?;
    submit(&mut device, &memory, command())?;
    run(&mut device)?;
    assert!(!device.take_interrupt());
    let mut ioc = command();
    ioc.flags.ioc = true;
    memory.write(
        MemoryAddress::new(0x1040),
        MemoryAlignment::ONE,
        &ioc.to_bytes()?,
    )?;
    device.write(
        COMMAND + CommandRegisters::PRODUCER.offset(),
        &2_u32.to_le_bytes(),
    )?;
    // When: IOC completes in an already non-empty queue.
    run(&mut device)?;
    // Then: IOC notifies once; unmask does not replay the earlier non-empty event.
    assert!(device.take_interrupt());
    device.write(mask, &0_u32.to_le_bytes())?;
    assert!(!device.take_interrupt());
    assert_eq!(
        read_word(&mut device, CONTROL + ChannelRegisters::IRQ_CAUSE.offset())?,
        3
    );
    device.write(
        RESPONSE + ResponseRegisters::CONSUMER.offset(),
        &2_u32.to_le_bytes(),
    )?;
    assert_eq!(
        read_word(&mut device, CONTROL + ChannelRegisters::IRQ_CAUSE.offset())?,
        0
    );
    Ok(())
}

#[test]
fn error_response_stops_the_channel_even_after_consumption() -> TestResult {
    // Given: mismatched endpoints followed by a valid command.
    let (mut device, memory) = setup(0x1100, 2)?;
    device.write(
        CONTROL + ChannelRegisters::IRQ_MASK.offset(),
        &3_u32.to_le_bytes(),
    )?;
    let mut invalid = command();
    invalid.destination = pdma_model_contract::Endpoint::linear(0x1300, 5);
    submit(&mut device, &memory, invalid)?;
    run(&mut device)?;
    device.write(
        RESPONSE + ResponseRegisters::CONSUMER.offset(),
        &1_u32.to_le_bytes(),
    )?;
    memory.write(
        MemoryAddress::new(0x1040),
        MemoryAlignment::ONE,
        &command().to_bytes()?,
    )?;
    device.write(
        COMMAND + CommandRegisters::PRODUCER.offset(),
        &2_u32.to_le_bytes(),
    )?;
    // When: software tries to continue without reset.
    let result = run(&mut device);
    // Then: no second command is fetched and the error cause stays asserted.
    assert!(result.is_err());
    assert_eq!(
        read_word(&mut device, COMMAND + CommandRegisters::CONSUMER.offset())?,
        1
    );
    assert!(device.irq());
    assert_eq!(
        read_word(&mut device, CONTROL + ChannelRegisters::IRQ_CAUSE.offset())?,
        4
    );
    Ok(())
}

#[test]
fn fatal_notifies_even_when_response_cause_is_already_active() -> TestResult {
    // Given: one unread response whose notification has already been delivered.
    let (mut device, memory) = setup(0x1100, 2)?;
    submit(&mut device, &memory, command())?;
    run(&mut device)?;
    assert!(device.take_interrupt());
    let mut malformed = command().to_bytes()?;
    malformed[8] = 255;
    memory.write(MemoryAddress::new(0x1040), MemoryAlignment::ONE, &malformed)?;
    device.write(
        COMMAND + CommandRegisters::PRODUCER.offset(),
        &2_u32.to_le_bytes(),
    )?;
    // When: a new command fetch fails while response non-empty remains true.
    assert!(run(&mut device).is_err());
    // Then: an independent fatal event is emitted with both causes visible.
    assert!(device.take_interrupt());
    assert_eq!(
        read_word(&mut device, CONTROL + ChannelRegisters::IRQ_CAUSE.offset())?,
        9
    );
    Ok(())
}

#[test]
fn ioc_cause_clears_at_latest_ioc_not_at_ring_empty() -> TestResult {
    // Given: two IOC responses followed by an ordinary response.
    let (mut device, memory) = setup(0x1100, 4)?;
    for producer in 0..3_u32 {
        let mut input = command();
        input.flags.ioc = producer < 2;
        memory.write(
            MemoryAddress::new(0x1000 + u64::from(producer) * 64),
            MemoryAlignment::ONE,
            &input.to_bytes()?,
        )?;
        device.write(
            COMMAND + CommandRegisters::PRODUCER.offset(),
            &(producer + 1).to_le_bytes(),
        )?;
        run(&mut device)?;
        assert_eq!(device.take_interrupt(), producer < 2);
    }
    // When: software consumes each IOC but leaves the last ordinary response unread.
    for (consumer, cause) in [(1_u32, 3), (2, 1)] {
        device.write(
            RESPONSE + ResponseRegisters::CONSUMER.offset(),
            &consumer.to_le_bytes(),
        )?;
        // Then: IOC persists through the first consumption and clears exactly at the last IOC.
        assert_eq!(
            read_word(&mut device, CONTROL + ChannelRegisters::IRQ_CAUSE.offset())?,
            cause
        );
    }
    assert!(!device.take_interrupt());
    Ok(())
}
