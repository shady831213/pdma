//! Selected reset preserves every other channel's retained hardware work.

use hardware_interface_pipeline::{Operation, StepOutcome};
use hardware_interface_runtime::{
    MemoryAccess, MemoryAddress, MemoryAlignment, MemoryRegion, SharedMemory,
};
use pdma_model_contract::{
    ChannelBlock, ChannelId, ChannelRegisters, DeviceControl, DeviceRegisters, ResponseRegisters,
};
use pdma_model_device::{Device, Error};

mod fairness;
mod fan_in_inputs;
mod reset_boundaries;
mod response_routing;
mod shared;
/// Shared raw-bus fixtures for the multi-channel boundary tests.
pub mod support;
use support::{TestResult, configure, submit, word};

#[test]
fn hardware_count_survives_both_reset_scopes_and_is_read_only() -> TestResult {
    // Given
    let mut device = Device::new(SharedMemory::new(), 3)?;
    let count = DeviceRegisters::CONTROL.offset() + DeviceControl::CHANNEL_NUM.offset();
    assert_eq!(word(&mut device, count)?, 3);
    // When
    let write = device.write(count, &1_u32.to_le_bytes());
    // Then
    assert_eq!(write, Err(Error::Access));
    for reset in [
        ChannelId::new(1)
            .register_offset()
            .ok_or("channel offset")?
            + ChannelBlock::CONTROL.offset()
            + ChannelRegisters::RESET.offset(),
        DeviceRegisters::CONTROL.offset() + DeviceControl::RESET.offset(),
    ] {
        device.write(reset, &1_u32.to_le_bytes())?;
        assert_eq!(word(&mut device, count)?, 3);
    }
    assert_eq!(
        device.read(
            ChannelId::new(3).register_offset().ok_or("offset")?,
            &mut [0; 8]
        ),
        Err(Error::Unmapped)
    );
    Ok(())
}

#[test]
fn reset_during_copy_does_not_restart_or_stop_other_channels() -> TestResult {
    // Given: three active copies and the first committed chunk on A.
    let memory = SharedMemory::new();
    memory
        .prepare_regions(&[MemoryRegion::try_new(
            MemoryAddress::new(0x1000),
            0x3000,
            MemoryAlignment::ONE,
        )?])?
        .try_install(&memory)?;
    let mut device = Device::new(memory.clone(), 3)?;
    for index in 0..3 {
        configure(&mut device, index)?;
        memory.write(
            MemoryAddress::new(0x1200 + u64::try_from(index)? * 0x1000),
            MemoryAlignment::ONE,
            &[7; 512],
        )?;
        submit(&mut device, index, u64::try_from(index)?)?;
    }
    let mut hardware = device.multi_channel();
    let mut operation = Operation::new(());
    let mut committed = false;
    for _ in 0..256 {
        let _ = operation.step(&mut hardware, &mut device);
        if memory.read_typed::<[u8; 1]>(MemoryAddress::new(0x1400), MemoryAlignment::ONE)? == [7] {
            committed = true;
            break;
        }
    }
    assert!(committed);
    // When: A is reset while B and C retain their progress.
    device.write(
        ChannelId::new(0).register_offset().ok_or("offset")?
            + ChannelBlock::CONTROL.offset()
            + ChannelRegisters::RESET.offset(),
        &1_u32.to_le_bytes(),
    )?;
    for _ in 0..512 {
        assert!(!matches!(
            operation.step(&mut hardware, &mut device),
            StepOutcome::Complete(Err(_))
        ));
    }
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
                ChannelId::new(index).register_offset().ok_or("offset")?
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
            ChannelId::new(0).register_offset().ok_or("offset")?
                + ChannelBlock::RESPONSE.offset()
                + ResponseRegisters::PRODUCER.offset()
        )?,
        0
    );
    configure(&mut device, 0)?;
    submit(&mut device, 0, 99)?;
    for _ in 0..512 {
        let _ = operation.step(&mut hardware, &mut device);
    }
    assert_eq!(
        word(
            &mut device,
            ChannelId::new(0).register_offset().ok_or("offset")?
                + ChannelBlock::RESPONSE.offset()
                + ResponseRegisters::PRODUCER.offset()
        )?,
        1
    );
    Ok(())
}
