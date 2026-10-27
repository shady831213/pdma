//! Register buses and resident DMA fixtures for control tests.

use hardware_interface_io::memory::MemoryAccessError;
use pdma_model_contract::dma::{
    ChannelBlock, ChannelConfiguration, ChannelId, ChannelRegisters, Command, CommandRegisters,
    CopyCommandDestinationValues, CopyCommandSourceValues, EventRegisters, LinearEndpoint,
};

use hardware_interface_io::memory::{
    MemoryAccess, MemoryAddress, MemoryAlignment, SharedMemoryAccess,
};
use hardware_interface_io::memory::{MemoryRegion, SharedMemory};
use hardware_interface_io::register::RegisterAccess;
use hardware_interface_pipeline::{HardwareDriver, StepOutcome};
use pdma_model_dma::{Dma, Hardware};
use std::sync::{Arc, Mutex};

type Driver = HardwareDriver<Hardware, Dma<SharedMemoryAccess<(), MemoryAccessError>>>;
pub(super) struct Bus(pub(super) Mutex<Dma<SharedMemoryAccess<(), MemoryAccessError>>>);
pub(super) struct Relocated(pub(super) Bus);

impl Bus {
    pub(super) fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let storage = Arc::new(SharedMemory::new());
        storage.add_regions(&[MemoryRegion::try_new(
            MemoryAddress::new(0x1000),
            0x2000,
            MemoryAlignment::ONE,
        )?])?;
        Ok(Self(Mutex::new(Dma::new(
            SharedMemoryAccess::from(storage),
            2,
        )?)))
    }

    pub(super) fn resident() -> Result<(Self, Hardware, Driver), Box<dyn std::error::Error>> {
        let bus = Self::new()?;
        let (hardware, driver) = {
            let mut dma = bus.0.lock().map_err(|_| "poisoned device lock")?;
            dma.write(
                ChannelId::new(0).register_offset(2).ok_or("peer")?
                    + ChannelBlock::CONTROL.offset()
                    + ChannelRegisters::CONFIG.offset()
                    + ChannelConfiguration::RING_CONTEXT.offset(),
                &37_u32.to_le_bytes(),
            )?;
            dma.memory.write_buffer(
                MemoryAddress::new(0x1000),
                MemoryAlignment::ONE,
                &Command::Copy(pdma_model_contract::dma::CopyCommand {
                    id: 13,
                    copy_length: 0,
                    sg_context: 0,
                    source: CopyCommandSourceValues::Linear(LinearEndpoint {
                        address: 0,
                        offset: 0,
                    }),
                    destination: CopyCommandDestinationValues::Linear(LinearEndpoint {
                        address: 0,
                        offset: 0,
                    }),
                    ..pdma_model_contract::dma::CopyCommand::ZERO
                })
                .to_bytes()?,
            )?;
            let base = ChannelId::new(1).register_offset(2).ok_or("channel")?;
            for (offset, address) in [
                (
                    ChannelBlock::COMMAND.offset() + CommandRegisters::BASE.offset(),
                    0x1000_u64,
                ),
                (
                    ChannelBlock::EVENT.offset() + EventRegisters::BASE.offset(),
                    0x2000_u64,
                ),
            ] {
                dma.write(base + offset, &address.to_le_bytes())?;
            }
            for offset in [
                ChannelBlock::COMMAND.offset() + CommandRegisters::CAPACITY.offset(),
                ChannelBlock::EVENT.offset() + EventRegisters::CAPACITY.offset(),
                ChannelBlock::CONTROL.offset()
                    + ChannelRegisters::CONFIG.offset()
                    + ChannelConfiguration::ENABLE.offset(),
                ChannelBlock::COMMAND.offset() + CommandRegisters::PRODUCER.offset(),
            ] {
                dma.write(base + offset, &1_u32.to_le_bytes())?;
            }
            let mut hardware = dma.hardware();
            let mut driver = Driver::new();
            let mut accepted = false;
            for _ in 0..128 {
                assert!(matches!(
                    driver.step(&mut hardware, &mut dma),
                    Ok(StepOutcome::InProgress)
                ));
                let mut bytes = [0; 4];
                dma.read(
                    base + ChannelBlock::COMMAND.offset() + CommandRegisters::CONSUMER.offset(),
                    &mut bytes,
                )?;
                if u32::from_le_bytes(bytes) == 1 {
                    accepted = true;
                    break;
                }
            }
            drop(dma);
            assert!(accepted);
            (hardware, driver)
        };
        Ok((bus, hardware, driver))
    }

    pub(super) fn advance(
        &self,
        hardware: &mut Hardware,
        driver: &mut Driver,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut dma = self.0.lock().map_err(|_| "poisoned device lock")?;
        for _ in 0..128 {
            assert!(matches!(
                driver.step(hardware, &mut dma),
                Ok(StepOutcome::InProgress)
            ));
        }
        Ok(())
    }

    pub(super) async fn busy(&self, channel: usize) -> Result<u32, Box<dyn std::error::Error>> {
        let offset = ChannelId::new(channel)
            .admin_offset(2)
            .ok_or("channel offset overflow")?;
        let mut bytes = [0; 4];
        self.read(u32::try_from(offset)?, &mut bytes).await?;
        Ok(u32::from_le_bytes(bytes))
    }

    pub(super) async fn ring_context(
        &self,
        channel: usize,
    ) -> Result<u32, Box<dyn std::error::Error>> {
        let offset = ChannelId::new(channel)
            .register_offset(2)
            .ok_or("channel")?
            + ChannelBlock::CONTROL.offset()
            + ChannelRegisters::CONFIG.offset()
            + ChannelConfiguration::RING_CONTEXT.offset();
        let mut bytes = [0; 4];
        self.read(u32::try_from(offset)?, &mut bytes).await?;
        Ok(u32::from_le_bytes(bytes))
    }
}

impl RegisterAccess for Bus {
    type Error = pdma_model_dma::Error;

    async fn read(&self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        self.0.lock().map_err(|_| Self::Error::Unmapped)?.read(
            usize::try_from(offset).map_err(|_| Self::Error::Unmapped)?,
            bytes,
        )
    }

    async fn write(&self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
        self.0.lock().map_err(|_| Self::Error::Unmapped)?.write(
            usize::try_from(offset).map_err(|_| Self::Error::Unmapped)?,
            bytes,
        )
    }
}

impl RegisterAccess for Relocated {
    type Error = pdma_model_dma::Error;

    async fn read(&self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
        self.0
            .read(
                offset.checked_sub(0x4000).ok_or(Self::Error::Unmapped)?,
                bytes,
            )
            .await
    }

    async fn write(&self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
        self.0
            .write(
                offset.checked_sub(0x4000).ok_or(Self::Error::Unmapped)?,
                bytes,
            )
            .await
    }
}
