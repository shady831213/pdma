use hardware_interface_pipeline::{HardwareDriver, StepOutcome};
use hardware_interface_runtime::{
    MemoryAccess, MemoryAddress, MemoryAlignment, MemoryRegion, SharedMemory,
};
use pdma_model_contract::dma::{
    ChannelBlock, ChannelId, ChannelRegisters, Command, CommandRegisters, Response,
    ResponseRegisters, SignalCommand, SignalOperation, SignalOrder, SignalTarget, WaitCommand,
    WaitCondition, WaitOrder,
};
use pdma_model_dma::{Dma, Hardware};

pub(super) type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
pub(super) const SIGNAL: u64 = 0x4000;
pub(super) const COUNT: usize = 2;

pub(super) struct Fixture<M: MemoryAccess = SharedMemory>
where
    M::Error: Into<pdma_model_dma::MemoryFailure>,
{
    pub dma: Dma<M>,
    pub memory: SharedMemory,
    pub hardware: Hardware,
    driver: HardwareDriver<Hardware, Dma<M>>,
}

impl Fixture {
    pub(super) fn new() -> Result<Self> {
        let memory = SharedMemory::new();
        Self::with_backend(memory.clone(), memory)
    }
}

impl<M: MemoryAccess> Fixture<M>
where
    M::Error: Into<pdma_model_dma::MemoryFailure>,
{
    pub(super) fn with_backend(memory: SharedMemory, backend: M) -> Result<Self> {
        memory.add_regions(&[MemoryRegion::try_new(
            MemoryAddress::new(0x1000),
            0x4000,
            MemoryAlignment::ONE,
        )?])?;
        let mut dma = Dma::new(backend, COUNT)?;
        for id in 0..COUNT {
            let base = offset(id)?;
            for (register, value) in [
                (
                    ChannelBlock::COMMAND.offset() + CommandRegisters::BASE.offset(),
                    command_base(id)?,
                ),
                (
                    ChannelBlock::RESPONSE.offset() + ResponseRegisters::BASE.offset(),
                    response_base(id)?,
                ),
            ] {
                dma.write(base + register, &value.to_le_bytes())?;
            }
            for (register, value) in [
                (
                    ChannelBlock::COMMAND.offset() + CommandRegisters::CAPACITY.offset(),
                    4_u32,
                ),
                (
                    ChannelBlock::RESPONSE.offset() + ResponseRegisters::CAPACITY.offset(),
                    4,
                ),
                (
                    ChannelBlock::CONTROL.offset() + ChannelRegisters::ENABLE.offset(),
                    1,
                ),
            ] {
                dma.write(base + register, &value.to_le_bytes())?;
            }
        }
        let hardware = dma.hardware();
        Ok(Self {
            dma,
            memory,
            hardware,
            driver: HardwareDriver::new(),
        })
    }

    pub(super) fn submit(&mut self, channel: usize, commands: &[Command]) -> Result {
        for (index, command) in commands.iter().enumerate() {
            self.memory.write(
                MemoryAddress::new(
                    command_base(channel)? + u64::try_from(index * Command::RECORD_SIZE)?,
                ),
                MemoryAlignment::ONE,
                &command.to_bytes()?,
            )?;
        }
        self.dma.write(
            offset(channel)? + ChannelBlock::COMMAND.offset() + CommandRegisters::PRODUCER.offset(),
            &u32::try_from(commands.len())?.to_le_bytes(),
        )?;
        Ok(())
    }

    pub(super) fn tick(&mut self, count: usize) -> Result {
        for _ in 0..count {
            assert!(matches!(
                self.driver.step(&mut self.hardware, &mut self.dma)?,
                StepOutcome::InProgress
            ));
        }
        Ok(())
    }

    pub(super) fn register(&mut self, channel: usize, register: usize) -> Result<u32> {
        let mut bytes = [0; 4];
        self.dma.read(offset(channel)? + register, &mut bytes)?;
        Ok(u32::from_le_bytes(bytes))
    }

    pub(super) fn produced(&mut self, channel: usize) -> Result<u32> {
        self.register(
            channel,
            ChannelBlock::RESPONSE.offset() + ResponseRegisters::PRODUCER.offset(),
        )
    }

    pub(super) fn response(&self, channel: usize, index: usize) -> Result<Response> {
        let mut bytes = [0; Response::RECORD_SIZE];
        self.memory.read(
            MemoryAddress::new(
                response_base(channel)? + u64::try_from(index * Response::RECORD_SIZE)?,
            ),
            MemoryAlignment::ONE,
            &mut bytes,
        )?;
        Ok(Response::from_bytes(&bytes)?)
    }

    pub(super) fn value(&self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.memory.read_typed::<[u8; 8]>(
            MemoryAddress::new(SIGNAL),
            MemoryAlignment::ONE,
        )?))
    }

    pub(super) fn store(&self, value: u64) -> Result {
        self.memory.write(
            MemoryAddress::new(SIGNAL),
            MemoryAlignment::ONE,
            &value.to_le_bytes(),
        )?;
        Ok(())
    }
}

pub(super) fn offset(channel: usize) -> Result<usize> {
    ChannelId::new(channel)
        .register_offset(COUNT)
        .ok_or_else(|| "channel offset".into())
}
fn command_base(channel: usize) -> Result<u64> {
    Ok(0x1000 + u64::try_from(channel)? * 0x400)
}
fn response_base(channel: usize) -> Result<u64> {
    Ok(0x2000 + u64::try_from(channel)? * 0x400)
}

pub(super) const fn signal(id: u64, operation: SignalOperation, value: u64) -> Command {
    Command::signal(
        id,
        SignalCommand {
            target: SignalTarget {
                address: SIGNAL,
                context: 1,
            },
            operation,
            order: SignalOrder::Relaxed,
            value,
            expected: 0,
        },
    )
}
pub(super) const fn wait(id: u64, compare: u64) -> Command {
    Command::wait(
        id,
        WaitCommand {
            target: SignalTarget {
                address: SIGNAL,
                context: 1,
            },
            condition: WaitCondition::Equal,
            order: WaitOrder::Acquire,
            compare,
        },
    )
}
