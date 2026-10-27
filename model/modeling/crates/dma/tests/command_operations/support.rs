use hardware_interface_io::memory::{MemoryAccess, MemoryAddress, MemoryAlignment};
use hardware_interface_io::memory::{MemoryRegion, SharedMemory};
use hardware_interface_pipeline::HardwareDriver;
use pdma_model_contract::dma::{
    CapabilitiesValues, ChannelBlock, ChannelConfiguration, ChannelId, ChannelInterrupts,
    ChannelRegisters, Command, CommandHeader, CommandRegisters, EventRegisters, FenceCommand,
    Opcode, WordTarget, WordTargetWidth, WordWrite,
};
use pdma_model_dma::{Dma, DmaConfig, Hardware};

pub(super) type TestResult = Result<(), Box<dyn std::error::Error>>;

pub(super) struct Rig<M: MemoryAccess = SharedMemory>
where
    M::Error: Into<pdma_model_dma::MemoryFailure>,
{
    pub(super) dma: Dma<M>,
    pub(super) memory: SharedMemory,
    hardware: Hardware,
    driver: HardwareDriver<Hardware, Dma<M>>,
    pub(super) bases: Vec<usize>,
}

impl Rig {
    pub(super) fn new(
        channels: usize,
        capabilities: u32,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let memory = SharedMemory::new();
        memory.add_regions(&[MemoryRegion::try_new(
            MemoryAddress::new(0x1000),
            0xf000,
            MemoryAlignment::ONE,
        )?])?;
        let dma = Dma::with_config(
            memory.clone(),
            DmaConfig {
                channels,
                capabilities: CapabilitiesValues::new(capabilities),
            },
        )?;
        Self::assemble(dma, memory, channels)
    }
}

impl<M: MemoryAccess> Rig<M>
where
    M::Error: Into<pdma_model_dma::MemoryFailure>,
{
    pub(super) fn assemble(
        mut dma: Dma<M>,
        memory: SharedMemory,
        channels: usize,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut bases = Vec::new();
        for index in 0..channels {
            let base = ChannelId::new(index)
                .register_offset(channels)
                .ok_or("channel offset overflow")?;
            bases.push(base);
            let ring = 0x1000 + u64::try_from(index)? * 0x1000;
            dma.write(
                base + ChannelBlock::COMMAND.offset() + CommandRegisters::BASE.offset(),
                &ring.to_le_bytes(),
            )?;
            dma.write(
                base + ChannelBlock::EVENT.offset() + EventRegisters::BASE.offset(),
                &(ring + 0x800).to_le_bytes(),
            )?;
            for (offset, value) in [
                (
                    ChannelBlock::COMMAND.offset() + CommandRegisters::CAPACITY.offset(),
                    8_u32,
                ),
                (
                    ChannelBlock::EVENT.offset() + EventRegisters::CAPACITY.offset(),
                    2,
                ),
                (
                    ChannelBlock::CONTROL.offset()
                        + ChannelRegisters::IRQ.offset()
                        + ChannelInterrupts::ENABLE.offset(),
                    1,
                ),
                (
                    ChannelBlock::CONTROL.offset()
                        + ChannelRegisters::CONFIG.offset()
                        + ChannelConfiguration::ENABLE.offset(),
                    1,
                ),
            ] {
                dma.write(base + offset, &value.to_le_bytes())?;
            }
        }
        let hardware = dma.hardware();
        Ok(Self {
            dma,
            memory,
            hardware,
            driver: HardwareDriver::new(),
            bases,
        })
    }

    pub(super) fn submit(&mut self, channel: usize, commands: &[Command]) -> TestResult {
        let base = *self.bases.get(channel).ok_or("channel missing")?;
        let ring = 0x1000 + u64::try_from(channel)? * 0x1000;
        for (index, command) in commands.iter().enumerate() {
            self.memory.write(
                MemoryAddress::new(ring + u64::try_from(index * Command::RECORD_SIZE)?),
                MemoryAlignment::ONE,
                &command.to_bytes()?,
            )?;
        }
        self.dma.write(
            base + ChannelBlock::COMMAND.offset() + CommandRegisters::PRODUCER.offset(),
            &u32::try_from(commands.len())?.to_le_bytes(),
        )?;
        Ok(())
    }

    pub(super) fn drive(&mut self) -> TestResult {
        for _ in 0..512 {
            let _ = self.driver.step(&mut self.hardware, &mut self.dma)?;
        }
        Ok(())
    }

    pub(super) fn step(&mut self) -> TestResult {
        let _ = self.driver.step(&mut self.hardware, &mut self.dma)?;
        Ok(())
    }

    pub(super) fn word(&self, address: u64) -> Result<u64, Box<dyn std::error::Error>> {
        let mut bytes = [0; 8];
        self.memory.read(
            MemoryAddress::new(address),
            MemoryAlignment::ONE,
            &mut bytes,
        )?;
        Ok(u64::from_le_bytes(bytes))
    }

    pub(super) fn register(
        &mut self,
        channel: usize,
        offset: usize,
    ) -> Result<u32, Box<dyn std::error::Error>> {
        let mut bytes = [0; 4];
        self.dma.read(
            *self.bases.get(channel).ok_or("channel missing")? + offset,
            &mut bytes,
        )?;
        Ok(u32::from_le_bytes(bytes))
    }
}

pub(super) const fn target(address: u64) -> WordTarget {
    WordTarget {
        address,
        context: 0,
        width: WordTargetWidth::Bits64,
    }
}

pub(super) const fn fence(address: u64) -> Command {
    Command::Fence(FenceCommand {
        header: CommandHeader {
            opcode: Opcode::Fence,
        },
        id: 90,
        word: WordWrite {
            target: target(address),
            value: 0xfeed,
        },
    })
}

pub(super) const EVENT_PRODUCER: usize =
    ChannelBlock::EVENT.offset() + EventRegisters::PRODUCER.offset();
pub(super) const CAUSE: usize = ChannelBlock::CONTROL.offset()
    + ChannelRegisters::IRQ.offset()
    + ChannelInterrupts::CAUSE.offset();
