//! MMIO rejection and side-effect boundaries.

/// Shared raw-bus fixtures.
pub mod support;

use pdma_model_contract::{
    ChannelBlock, ChannelRegisters, DeviceControl, DeviceRegisters, ResponseRegisters,
};
use pdma_model_device::Error;
use support::{TestResult, command, read_word, run, setup, submit};

const CONTROL: usize = DeviceRegisters::CHANNELS_OFFSET + ChannelBlock::CONTROL.offset();
const RESPONSE: usize = DeviceRegisters::CHANNELS_OFFSET + ChannelBlock::RESPONSE.offset();

#[test]
fn rejected_root_access_preserves_channels_and_pending_irq() -> TestResult {
    // Given: an enabled channel with an unread completion and pending notification.
    let (mut device, memory) = setup(0x1100, 2)?;
    submit(&mut device, &memory, command())?;
    run(&mut device)?;
    let reset = DeviceRegisters::CONTROL.offset() + DeviceControl::RESET.offset();
    let count = DeviceRegisters::CONTROL.offset() + DeviceControl::CHANNEL_NUM.offset();
    for (offset, bytes, expected) in [
        (reset, &[1_u8][..], Error::Width),
        (reset, &[2, 0, 0, 0][..], Error::ReservedBits),
        (count, &[1, 0, 0, 0][..], Error::Access),
        (count + 1, &[0; 4][..], Error::Unmapped),
        (DeviceRegisters::extent(1)?, &[0; 4][..], Error::Unmapped),
        (usize::MAX, &[0; 4][..], Error::Unmapped),
    ] {
        // When: a rejected root access carries a potential reset or another write.
        let result = device.write(offset, bytes);
        // Then: no fallback performs a write or reset on any channel.
        assert_eq!(result, Err(expected));
        assert_eq!(
            read_word(&mut device, CONTROL + ChannelRegisters::ENABLE.offset())?,
            1
        );
        assert_eq!(
            read_word(&mut device, RESPONSE + ResponseRegisters::PRODUCER.offset())?,
            1
        );
        assert!(device.irq());
    }
    Ok(())
}

#[test]
fn rejected_reads_preserve_destination_and_pending_notification() -> TestResult {
    // Given: a pending completion notification and a sentinel destination.
    let (mut device, memory) = setup(0x1100, 2)?;
    submit(&mut device, &memory, command())?;
    run(&mut device)?;
    for (offset, width, expected) in [
        (CONTROL + ChannelRegisters::RESET.offset(), 1, Error::Width),
        (RESPONSE + ResponseRegisters::BASE.offset(), 4, Error::Width),
        (RESPONSE + 1, 4, Error::Unmapped),
        (usize::MAX, 4, Error::Unmapped),
        (CONTROL + ChannelRegisters::ENABLE.offset(), 0, Error::Width),
    ] {
        let mut destination = [0xa5; 4];
        // When: the bus rejects the read.
        let result = device.read(offset, destination.get_mut(..width).ok_or("width")?);
        // Then: neither caller bytes nor notification ownership change.
        assert_eq!(result, Err(expected));
        assert_eq!(destination, [0xa5; 4]);
        assert!(device.irq());
    }
    Ok(())
}

#[test]
fn rejected_consumption_preserves_ioc_and_consumer() -> TestResult {
    // Given: one unconsumed IOC completion and its delivered notification.
    let (mut device, memory) = setup(0x1100, 2)?;
    let mut input = command();
    input.flags.ioc = true;
    submit(&mut device, &memory, input)?;
    run(&mut device)?;
    assert!(device.take_interrupt());
    // When: software attempts to consume beyond the produced response.
    let result = device.write(
        RESPONSE + ResponseRegisters::CONSUMER.offset(),
        &2_u32.to_le_bytes(),
    );
    // Then: rejection does not clear the outstanding IOC or publish a cursor.
    assert_eq!(result, Err(Error::Cursor));
    assert_eq!(
        read_word(&mut device, RESPONSE + ResponseRegisters::CONSUMER.offset())?,
        0
    );
    assert_eq!(
        read_word(&mut device, CONTROL + ChannelRegisters::IRQ_CAUSE.offset())?,
        3
    );
    assert!(!device.irq());
    Ok(())
}
