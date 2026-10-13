//! Generation-bound task handoff through the shared execution pipeline.

use hardware_interface_pipeline::StepOutcome;
use hardware_interface_runtime::SharedMemory;
use pdma_model_contract::dma::ChannelId;
use pdma_model_dma::Dma;

#[test]
fn idle_or_device_fatal_keeps_external_driver_in_progress() -> Result<(), Box<dyn std::error::Error>>
{
    // Given: a three-channel instance with no runnable command.
    let mut device = Dma::new(SharedMemory::new(), 3)?;
    let mut hardware = device.service();
    let mut operation = hardware_interface_pipeline::HardwareDriver::new();
    for fatal in [false, true] {
        if fatal {
            device.signal_fatal();
        }
        // When: every channel receives one arbitration attempt.
        assert!(matches!(
            operation.step(&mut hardware, &mut device)?,
            StepOutcome::InProgress
        ));
        assert!(matches!(
            operation.step(&mut hardware, &mut device)?,
            StepOutcome::InProgress
        ));
        // Then: empty service transactions keep external hardware driving active.
        assert!(matches!(
            operation.step(&mut hardware, &mut device)?,
            StepOutcome::InProgress
        ));
    }
    assert!(device.channel_context(ChannelId::new(3)).is_err());
    Ok(())
}

#[test]
fn invalid_instance_count_is_rejected_before_allocation() {
    // Given: counts outside the contract or bus address space.
    for count in [0, usize::MAX, usize::try_from(u32::MAX).expect("u32 count")] {
        // When: constructing hardware with no DMA memory backing.
        let result = Dma::new(SharedMemory::new(), count);
        // Then: no invalid instance is returned.
        assert!(result.is_err());
    }
}

#[test]
fn independently_sized_idle_devices_keep_external_driver_in_progress()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: independently configured one- and five-channel devices.
    for count in [1, 5] {
        let mut device = Dma::new(SharedMemory::new(), count)?;
        let mut arbiter = device.service();
        let mut operation = hardware_interface_pipeline::HardwareDriver::new();
        // When: repeatedly driving idle arbitration for each population.
        for _ in 0..128 {
            assert!(matches!(
                operation.step(&mut arbiter, &mut device)?,
                StepOutcome::InProgress
            ));
        }
        // Then: repeated empty service transactions keep external driving active.
        assert!(!operation.is_complete());
    }
    Ok(())
}
