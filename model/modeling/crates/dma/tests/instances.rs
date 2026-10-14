//! Instance sizing and quiescent shared hardware driving.

use hardware_interface_pipeline::{HardwareDriver, StepOutcome};
use hardware_interface_runtime::{
    MemoryAccess, MemoryAccessError, MemoryAddress, MemoryAlignment, SharedMemory,
};
use pdma_model_contract::dma::{
    ChannelBlock, ChannelId, ChannelRegisters, CommandRegisters, ResponseRegisters,
};
use pdma_model_dma::Dma;
use std::cell::Cell;

#[derive(Default)]
struct MemoryProbe(Cell<usize>);

impl MemoryAccess for MemoryProbe {
    type Error = MemoryAccessError;

    fn validate_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        _: MemoryAlignment,
        _: u64,
    ) -> Result<(), Self::Error> {
        self.0.set(self.0.get() + 1);
        Err(MemoryAccessError::Unmapped { address })
    }

    fn read_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        _: MemoryAlignment,
        _: &mut [u8],
    ) -> Result<(), Self::Error> {
        self.0.set(self.0.get() + 1);
        Err(MemoryAccessError::Unmapped { address })
    }

    fn write_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        _: MemoryAlignment,
        _: &[u8],
    ) -> Result<(), Self::Error> {
        self.0.set(self.0.get() + 1);
        Err(MemoryAccessError::Unmapped { address })
    }
}

#[test]
fn idle_or_fatal_devices_keep_driving_without_memory_or_queue_effects()
-> Result<(), Box<dyn std::error::Error>> {
    for count in [1, 3, 5] {
        for fatal in [false, true] {
            // Given: enabled empty channels, or queued commands behind a device fatal.
            let mut device = Dma::new(MemoryProbe::default(), count)?;
            let queued = u32::from(fatal);
            for index in 0..count {
                let base = ChannelId::new(index)
                    .register_offset(count)
                    .ok_or("channel offset overflow")?;
                for (offset, address) in [
                    (
                        ChannelBlock::COMMAND.offset() + CommandRegisters::BASE.offset(),
                        0x1000_u64,
                    ),
                    (
                        ChannelBlock::RESPONSE.offset() + ResponseRegisters::BASE.offset(),
                        0x2000,
                    ),
                ] {
                    device.write(base + offset, &address.to_le_bytes())?;
                }
                for (offset, value) in [
                    (
                        ChannelBlock::COMMAND.offset() + CommandRegisters::CAPACITY.offset(),
                        2_u32,
                    ),
                    (
                        ChannelBlock::RESPONSE.offset() + ResponseRegisters::CAPACITY.offset(),
                        2,
                    ),
                    (
                        ChannelBlock::CONTROL.offset() + ChannelRegisters::ENABLE.offset(),
                        1,
                    ),
                    (
                        ChannelBlock::COMMAND.offset() + CommandRegisters::PRODUCER.offset(),
                        queued,
                    ),
                ] {
                    device.write(base + offset, &value.to_le_bytes())?;
                }
            }
            if fatal {
                device.signal_fatal();
            }
            let mut hardware = device.service();
            let mut driver = HardwareDriver::new();

            // When: repeated ticks advance the complete hardware assembly.
            for _ in 0..128 {
                assert!(matches!(
                    driver.step(&mut hardware, &mut device)?,
                    StepOutcome::InProgress
                ));
            }

            // Then: driving stays live without accessing memory or advancing either ring.
            assert!(!driver.is_complete());
            assert_eq!(device.memory.0.get(), 0);
            for index in 0..count {
                let base = ChannelId::new(index)
                    .register_offset(count)
                    .ok_or("channel offset overflow")?;
                for (offset, expected) in [
                    (
                        ChannelBlock::COMMAND.offset() + CommandRegisters::PRODUCER.offset(),
                        queued,
                    ),
                    (
                        ChannelBlock::COMMAND.offset() + CommandRegisters::CONSUMER.offset(),
                        0,
                    ),
                    (
                        ChannelBlock::RESPONSE.offset() + ResponseRegisters::PRODUCER.offset(),
                        0,
                    ),
                    (
                        ChannelBlock::RESPONSE.offset() + ResponseRegisters::CONSUMER.offset(),
                        0,
                    ),
                ] {
                    let mut bytes = [0; 4];
                    device.read(base + offset, &mut bytes)?;
                    assert_eq!(u32::from_le_bytes(bytes), expected);
                }
            }
            assert!(device.channel_context(ChannelId::new(count)).is_err());
        }
    }
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
