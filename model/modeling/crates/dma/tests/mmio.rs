//! MMIO rejection and side-effect boundaries.

use pdma_model_contract::dma::{
    ChannelBlock, ChannelConfiguration, ChannelInterrupts, ChannelRegisters, Command,
    CommandHeader, CopyCommand, CopyCommandDestinationValues, CopyCommandSourceValues, DmaControl,
    DmaRegisters, EventRegisters, LinearEndpoint, Opcode, TrapCommand,
};

/// Shared raw-bus fixtures.
pub mod support;

use pdma_model_dma::Error;
use support::{TestResult, read_word, run, setup, submit};

const CONTROL: usize =
    pdma_model_contract::dma::ChannelBlock::ALIGNMENT + ChannelBlock::CONTROL.offset();
const EVENT: usize =
    pdma_model_contract::dma::ChannelBlock::ALIGNMENT + ChannelBlock::EVENT.offset();

#[test]
fn rejected_root_access_preserves_channels_and_pending_irq() -> TestResult {
    // Given: an enabled channel with an unread error event and pending notification.
    let (mut device, memory) = setup(0x1100, 2)?;
    submit(
        &mut device,
        &memory,
        Command::Copy(CopyCommand {
            id: 1,
            source: CopyCommandSourceValues::Linear(LinearEndpoint {
                address: 0,
                offset: 0,
            }),
            destination: CopyCommandDestinationValues::Linear(LinearEndpoint {
                address: 0x1300,
                offset: 0,
            }),
            copy_length: 4,
            sg_context: 0,
            ..pdma_model_contract::dma::CopyCommand::ZERO
        }),
    )?;
    run(&mut device)?;
    let reset = (pdma_model_contract::dma::DmaDiscovery::EXTENT
        + pdma_model_contract::dma::DmaAdmin::CONTROL.offset())
        + DmaControl::RESET.offset();
    let count = DmaRegisters::DISCOVERY.offset()
        + pdma_model_contract::dma::DmaDiscovery::CHANNEL_NUM.offset();
    for (offset, bytes, expected) in [
        (reset, &[1_u8][..], Error::Width),
        (reset, &[2, 0, 0, 0][..], Error::ReservedBits),
        (count, &[1, 0, 0, 0][..], Error::Access),
        (count + 1, &[0; 4][..], Error::Unmapped),
        (DmaRegisters::new(1)?.extent(), &[0; 4][..], Error::Unmapped),
        (usize::MAX, &[0; 4][..], Error::Unmapped),
    ] {
        // When: a rejected root access carries a potential reset or another write.
        let result = device.write(offset, bytes);
        // Then: no fallback performs a write or reset on any channel.
        assert_eq!(result, Err(expected));
        assert_eq!(
            read_word(
                &mut device,
                CONTROL + ChannelRegisters::CONFIG.offset() + ChannelConfiguration::ENABLE.offset()
            )?,
            1
        );
        assert_eq!(
            read_word(&mut device, EVENT + EventRegisters::PRODUCER.offset())?,
            1
        );
        assert!(device.irq());
    }
    Ok(())
}

#[test]
fn rejected_reads_preserve_destination_and_pending_notification() -> TestResult {
    // Given: a pending Trap notification and a sentinel destination.
    let (mut device, memory) = setup(0x1100, 2)?;
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
    for (offset, width, expected) in [
        (
            pdma_model_contract::dma::ChannelId::new(0)
                .admin_offset(1)
                .ok_or("admin offset")?,
            1,
            Error::Width,
        ),
        (EVENT + EventRegisters::BASE.offset(), 4, Error::Width),
        (EVENT + 1, 4, Error::Unmapped),
        (usize::MAX, 4, Error::Unmapped),
        (
            CONTROL + ChannelRegisters::CONFIG.offset() + ChannelConfiguration::ENABLE.offset(),
            0,
            Error::Width,
        ),
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
fn rejected_consumption_preserves_trap_and_consumer() -> TestResult {
    // Given: one sticky Trap cause and its delivered notification.
    let (mut device, memory) = setup(0x1100, 2)?;
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
    assert!(device.take_interrupt());
    // When: software attempts to consume beyond the empty event ring.
    let result = device.write(
        EVENT + EventRegisters::CONSUMER.offset(),
        &2_u32.to_le_bytes(),
    );
    // Then: rejection does not clear the sticky Trap or publish a cursor.
    assert_eq!(result, Err(Error::Cursor));
    assert_eq!(
        read_word(&mut device, EVENT + EventRegisters::CONSUMER.offset())?,
        0
    );
    assert_eq!(
        read_word(
            &mut device,
            CONTROL + ChannelRegisters::IRQ.offset() + ChannelInterrupts::CAUSE.offset()
        )?,
        2
    );
    assert!(!device.irq());
    Ok(())
}
