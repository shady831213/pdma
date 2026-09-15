//! Hardware reset boundaries, including retained execution state.

/// Shared raw-bus fixtures.
pub mod support;

use hardware_interface_pipeline::{Operation, RunOutcome, StepExecutor};
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment};
use pdma_model_contract::{
    ChannelRegisters, CommandRegisters, DeviceControl, DeviceRegisters, Endpoint, ResponseRegisters,
};
use pdma_model_dma::ResetControl;
use support::{TestResult, command, read_word, setup, submit};

#[test]
fn reset_invalidates_a_retained_copy_without_rolling_back_memory() -> TestResult {
    // Given: an accepted operation and externally written payload bytes.
    let (mut device, memory) = setup(0x1100, 2)?;
    submit(&mut device, &memory, command())?;
    let mut hardware = device.command_loop();
    let mut operation = Operation::new(());
    let mut accepted = false;
    for _ in 0..16 {
        assert!(matches!(
            StepExecutor::new().step(&mut hardware, &mut operation, &mut device),
            Ok(RunOutcome::Progressed)
        ));
        if read_word(
            &mut device,
            DeviceRegisters::CHANNELS_OFFSET
                + pdma_model_contract::ChannelBlock::COMMAND.offset()
                + CommandRegisters::CONSUMER.offset(),
        )? == 1
        {
            accepted = true;
            break;
        }
    }
    assert!(accepted);
    memory.write(MemoryAddress::new(0x1300), MemoryAlignment::ONE, b"keep")?;
    // When: MMIO reset occurs before the retained operation resumes.
    device.write(
        DeviceRegisters::CHANNELS_OFFSET
            + pdma_model_contract::ChannelBlock::CONTROL.offset()
            + ChannelRegisters::RESET.offset(),
        &1_u32.to_le_bytes(),
    )?;
    let reset = DeviceRegisters::CHANNELS_OFFSET
        + pdma_model_contract::ChannelBlock::CONTROL.offset()
        + ChannelRegisters::RESET.offset();
    assert_eq!(read_word(&mut device, reset)?, 1);
    for _ in 0..32 {
        assert!(matches!(
            StepExecutor::new().step(&mut hardware, &mut operation, &mut device),
            Ok(RunOutcome::Progressed | RunOutcome::Pending(()))
        ));
        if read_word(&mut device, reset)? == 0 {
            break;
        }
    }
    // Then: stale work is discarded and external memory remains intact.
    assert_eq!(read_word(&mut device, reset)?, 0);
    assert_eq!(
        memory.read_typed::<[u8; 4]>(MemoryAddress::new(0x1300), MemoryAlignment::ONE)?,
        *b"keep"
    );
    assert_eq!(
        read_word(
            &mut device,
            DeviceRegisters::CHANNELS_OFFSET
                + pdma_model_contract::ChannelBlock::RESPONSE.offset()
                + ResponseRegisters::CAPACITY.offset()
        )?,
        0
    );
    assert!(!device.irq());
    Ok(())
}

#[test]
fn channel_reset_cannot_clear_device_fatal() -> TestResult {
    // Given: a device-wide internal hardware fault.
    let (mut device, _) = setup(0x1100, 2)?;
    device.write(
        DeviceRegisters::CONTROL.offset() + DeviceControl::IRQ_ENABLE.offset(),
        &1_u32.to_le_bytes(),
    )?;
    device.signal_fatal();
    // When: software resets only its channel.
    device.write(
        DeviceRegisters::CHANNELS_OFFSET
            + pdma_model_contract::ChannelBlock::CONTROL.offset()
            + ChannelRegisters::RESET.offset(),
        &1_u32.to_le_bytes(),
    )?;
    let reset = DeviceRegisters::CHANNELS_OFFSET
        + pdma_model_contract::ChannelBlock::CONTROL.offset()
        + ChannelRegisters::RESET.offset();
    assert_eq!(read_word(&mut device, reset)?, 1);
    let _ = Operation::new(()).step(&mut ResetControl, &mut device);
    assert_eq!(read_word(&mut device, reset)?, 0);
    // Then: device fatal survives until a whole-device reset.
    assert!(device.irq());
    assert_eq!(
        read_word(
            &mut device,
            DeviceRegisters::CONTROL.offset() + DeviceControl::FAULT.offset()
        )?,
        1
    );
    device.write(
        DeviceRegisters::CONTROL.offset() + DeviceControl::RESET.offset(),
        &1_u32.to_le_bytes(),
    )?;
    let reset = DeviceRegisters::CONTROL.offset() + DeviceControl::RESET.offset();
    assert_eq!(read_word(&mut device, reset)?, 1);
    let _ = Operation::new(()).step(&mut ResetControl, &mut device);
    assert_eq!(read_word(&mut device, reset)?, 0);
    assert!(!device.irq());
    assert_eq!(
        read_word(
            &mut device,
            DeviceRegisters::CONTROL.offset() + DeviceControl::FAULT.offset()
        )?,
        0
    );
    Ok(())
}

#[test]
fn reset_after_committed_chunk_discards_remaining_copy_after_reconfiguration() -> TestResult {
    for reset in [
        DeviceRegisters::CHANNELS_OFFSET
            + pdma_model_contract::ChannelBlock::CONTROL.offset()
            + ChannelRegisters::RESET.offset(),
        DeviceRegisters::CONTROL.offset() + DeviceControl::RESET.offset(),
    ] {
        // Given: the first half of a two-chunk DMA has really reached memory.
        let (mut device, memory) = setup(0x1100, 2)?;
        memory.write(MemoryAddress::new(0x1200), MemoryAlignment::ONE, &[7; 512])?;
        let mut input = command();
        input.source = Endpoint::linear(0x1200, 512);
        input.destination = Endpoint::linear(0x1600, 512);
        submit(&mut device, &memory, input)?;
        let mut hardware = device.command_loop();
        let mut operation = Operation::new(());
        let mut first_committed = false;
        let mut first_chunk = [0; 256];
        for _ in 0..64 {
            let _ = StepExecutor::new().step(&mut hardware, &mut operation, &mut device);
            memory.read(
                MemoryAddress::new(0x1600),
                MemoryAlignment::ONE,
                &mut first_chunk,
            )?;
            if first_chunk == [7; 256] {
                first_committed = true;
                break;
            }
        }
        assert!(first_committed);
        // When: software requests reset and waits for the retained hardware to finish cleanup.
        device.write(reset, &1_u32.to_le_bytes())?;
        assert_eq!(read_word(&mut device, reset)?, 1);
        for _ in 0..32 {
            assert!(matches!(
                StepExecutor::new().step(&mut hardware, &mut operation, &mut device),
                Ok(RunOutcome::Progressed | RunOutcome::Pending(()))
            ));
            if read_word(&mut device, reset)? == 0 {
                break;
            }
        }
        assert_eq!(read_word(&mut device, reset)?, 0);
        device.write(
            DeviceRegisters::CHANNELS_OFFSET
                + pdma_model_contract::ChannelBlock::COMMAND.offset()
                + CommandRegisters::BASE.offset(),
            &0x1000_u64.to_le_bytes(),
        )?;
        device.write(
            DeviceRegisters::CHANNELS_OFFSET
                + pdma_model_contract::ChannelBlock::RESPONSE.offset()
                + ResponseRegisters::BASE.offset(),
            &0x1100_u64.to_le_bytes(),
        )?;
        for offset in [
            DeviceRegisters::CHANNELS_OFFSET
                + pdma_model_contract::ChannelBlock::COMMAND.offset()
                + CommandRegisters::CAPACITY.offset(),
            DeviceRegisters::CHANNELS_OFFSET
                + pdma_model_contract::ChannelBlock::RESPONSE.offset()
                + ResponseRegisters::CAPACITY.offset(),
        ] {
            device.write(offset, &2_u32.to_le_bytes())?;
        }
        device.write(
            DeviceRegisters::CHANNELS_OFFSET
                + pdma_model_contract::ChannelBlock::CONTROL.offset()
                + ChannelRegisters::ENABLE.offset(),
            &1_u32.to_le_bytes(),
        )?;
        // Then: the prefix survives, the suffix is untouched, and new work can complete.
        memory.read(
            MemoryAddress::new(0x1600),
            MemoryAlignment::ONE,
            &mut first_chunk,
        )?;
        assert_eq!(first_chunk, [7; 256]);
        let mut last_chunk = [0; 256];
        memory.read(
            MemoryAddress::new(0x1700),
            MemoryAlignment::ONE,
            &mut last_chunk,
        )?;
        assert_eq!(last_chunk, [0; 256]);
        submit(&mut device, &memory, command())?;
        for _ in 0..128 {
            assert!(matches!(
                StepExecutor::new().step(&mut hardware, &mut operation, &mut device),
                Ok(RunOutcome::Progressed | RunOutcome::Pending(()))
            ));
        }
        assert_eq!(
            read_word(
                &mut device,
                DeviceRegisters::CHANNELS_OFFSET
                    + pdma_model_contract::ChannelBlock::RESPONSE.offset()
                    + ResponseRegisters::PRODUCER.offset()
            )?,
            1
        );
    }
    Ok(())
}
