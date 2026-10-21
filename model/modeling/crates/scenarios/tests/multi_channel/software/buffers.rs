use hardware_interface_runtime::{
    MemoryAccess, MemoryAddress, MemoryAlignment, SharedMemoryAccess, SharedMemoryAllocator,
};
use pdma_model_contract::dma::{ChannelId, Command, CompletionFlagsValues, Endpoint, Response};
use pdma_model_host::{ChannelConfig, RingConfig, Rings};

use crate::TestResult;

pub(super) struct Buffers {
    pub(super) config: ChannelConfig,
    pub(super) source: MemoryAddress,
    pub(super) destination: MemoryAddress,
    pub(super) length: usize,
    pub(super) value: u8,
}

impl Buffers {
    pub(super) fn allocate(allocator: &SharedMemoryAllocator, index: usize) -> TestResult<Self> {
        let command = allocator.allocate(
            4 * u64::try_from(Command::RECORD_SIZE)?,
            MemoryAlignment::try_new(u64::try_from(Command::RECORD_ALIGNMENT)?)?,
        )?;
        let response = allocator.allocate(
            u64::try_from(Response::RECORD_SIZE)?,
            MemoryAlignment::try_new(u64::try_from(Response::RECORD_ALIGNMENT)?)?,
        )?;
        let length = if index == 0 { 1024 } else { 16_384 };
        let source = allocator.allocate(u64::try_from(length)?, MemoryAlignment::ONE)?;
        let destination = allocator.allocate(2 * u64::try_from(length)?, MemoryAlignment::ONE)?;
        let value = 0x31 + u8::try_from(index)?;
        allocator
            .memory_access()
            .write(source, MemoryAlignment::ONE, &vec![value; length])?;
        Ok(Self {
            config: ChannelConfig {
                id: ChannelId::new(index),
                rings: Rings {
                    command: RingConfig::new(command.raw(), 4)?,
                    response: RingConfig::new(response.raw(), 1)?,
                },
            },
            source,
            destination,
            length,
            value,
        })
    }

    pub(super) fn command(&self, id: u64, second: bool) -> TestResult<Command> {
        let length = u64::try_from(self.length)?;
        let destination = self.destination.raw() + if second { length } else { 0 };
        Ok(Command {
            copy_length: length,
            sg_context: 0,
            id,
            source: Endpoint::linear(self.source.raw(), 0),
            destination: Endpoint::linear(destination, 0),
            flags: CompletionFlagsValues::default(),
        })
    }

    pub(super) fn byte_matches(
        &self,
        memory: &SharedMemoryAccess,
        offset: usize,
    ) -> TestResult<bool> {
        let address = self
            .destination
            .checked_add(u64::try_from(offset)?)
            .ok_or("destination byte address overflow")?;
        let mut byte = [0; 1];
        memory.read(address, MemoryAlignment::ONE, &mut byte)?;
        Ok(byte == [self.value])
    }

    pub(super) fn snapshot(&self, memory: &SharedMemoryAccess) -> TestResult<Vec<u8>> {
        let mut bytes = vec![0; self.length * 2];
        memory.read(self.destination, MemoryAlignment::ONE, &mut bytes)?;
        Ok(bytes)
    }
}
