//! Selected reset preserves every other channel's retained hardware work.

use hardware_interface_pipeline::{Operation, RunOutcome};
use hardware_interface_runtime::{
    MemoryAccess, MemoryAddress, MemoryAlignment, MemoryRegion, SharedMemory,
};
use pdma_model_contract::dma::{
    ChannelBlock, ChannelId, DmaControl, DmaRegisters, ResponseRegisters,
};
use pdma_model_dma::{Dma, Error, ResetControl};

mod command_grant;
mod context_abort;
mod fairness;
mod fan_in_inputs;
mod reset_boundaries;
mod response_routing;
mod scatter_gather_reset;
mod shared;
/// Shared raw-bus fixtures for the multi-channel boundary tests.
pub mod support;
use support::{TestResult, configure, submit, word};

#[test]
fn hardware_count_survives_both_reset_scopes_and_is_read_only() -> TestResult {
    // Given
    let mut device = Dma::new(SharedMemory::new(), 3)?;
    let count = DmaRegisters::DISCOVERY.offset()
        + pdma_model_contract::dma::DmaDiscovery::CHANNEL_NUM.offset();
    assert_eq!(word(&mut device, count)?, 3);
    // When
    let write = device.write(count, &1_u32.to_le_bytes());
    // Then
    assert_eq!(write, Err(Error::Access));
    for reset in [
        ChannelId::new(1).admin_offset(3).ok_or("channel offset")?,
        (pdma_model_contract::dma::DmaDiscovery::EXTENT
            + pdma_model_contract::dma::DmaAdmin::CONTROL.offset())
            + DmaControl::RESET.offset(),
    ] {
        device.write(reset, &1_u32.to_le_bytes())?;
        assert_eq!(word(&mut device, reset)?, 1);
        let _ = Operation::new(()).step(&mut ResetControl, &mut device);
        assert_eq!(word(&mut device, reset)?, 0);
        assert_eq!(word(&mut device, count)?, 3);
    }
    assert_eq!(
        device.read(DmaRegisters::new(3)?.extent(), &mut [0; 8]),
        Err(Error::Unmapped)
    );
    Ok(())
}

#[test]
fn reset_during_copy_does_not_restart_or_stop_other_channels() -> TestResult {
    // Given: three active copies and the first committed chunk on A.
    let memory = SharedMemory::new();
    memory.add_regions(&[MemoryRegion::try_new(
        MemoryAddress::new(0x1000),
        0x3000,
        MemoryAlignment::ONE,
    )?])?;
    let mut device = Dma::new(memory.clone(), 3)?;
    for index in 0..3 {
        configure(&mut device, index)?;
        memory.write(
            MemoryAddress::new(0x1200 + u64::try_from(index)? * 0x1000),
            MemoryAlignment::ONE,
            &[7; 512],
        )?;
        submit(&mut device, index, u64::try_from(index)?)?;
    }
    let mut hardware = device.command_loop();
    let mut operation = Operation::new(());
    let mut committed = false;
    for _ in 0..256 {
        let _ = operation.step(&mut hardware, &mut device)?;
        if memory.read_typed::<[u8; 1]>(MemoryAddress::new(0x1400), MemoryAlignment::ONE)? == [7] {
            committed = true;
            break;
        }
    }
    assert!(committed);
    // When: A is reset while B and C retain their progress.
    device.write(
        ChannelId::new(0).admin_offset(3).ok_or("offset")?,
        &1_u32.to_le_bytes(),
    )?;
    let reset = ChannelId::new(0).admin_offset(3).ok_or("offset")?;
    assert_eq!(word(&mut device, reset)?, 1);
    for _ in 0..512 {
        assert!(!matches!(
            operation.step(&mut hardware, &mut device)?,
            RunOutcome::Complete(Err(_))
        ));
    }
    assert_eq!(word(&mut device, reset)?, 0);
    // Then: B/C complete; A's uncommitted suffix and response remain absent.
    for index in 1..3 {
        let mut bytes = [0; 512];
        memory.read(
            MemoryAddress::new(0x1400 + u64::try_from(index)? * 0x1000),
            MemoryAlignment::ONE,
            &mut bytes,
        )?;
        assert_eq!(bytes, [7; 512]);
        assert_eq!(
            word(
                &mut device,
                ChannelId::new(index).register_offset(3).ok_or("offset")?
                    + ChannelBlock::RESPONSE.offset()
                    + ResponseRegisters::PRODUCER.offset()
            )?,
            1
        );
    }
    assert!(device.take_interrupt());
    assert!(!device.take_interrupt());
    assert_eq!(
        memory.read_typed::<[u8; 1]>(MemoryAddress::new(0x1500), MemoryAlignment::ONE)?,
        [0]
    );
    assert_eq!(
        word(
            &mut device,
            ChannelId::new(0).register_offset(3).ok_or("offset")?
                + ChannelBlock::RESPONSE.offset()
                + ResponseRegisters::PRODUCER.offset()
        )?,
        0
    );
    configure(&mut device, 0)?;
    submit(&mut device, 0, 99)?;
    for _ in 0..512 {
        let _ = operation.step(&mut hardware, &mut device)?;
    }
    assert_eq!(
        word(
            &mut device,
            ChannelId::new(0).register_offset(3).ok_or("offset")?
                + ChannelBlock::RESPONSE.offset()
                + ResponseRegisters::PRODUCER.offset()
        )?,
        1
    );
    Ok(())
}
