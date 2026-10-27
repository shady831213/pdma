use hardware_interface_io::memory::MemoryAccessError;
use pdma_model_contract::dma::{
    ChannelId, Command, CopyCommandDestinationValues, CopyCommandSourceValues, Event,
    LinearEndpoint,
};

use hardware_interface_io::memory::SharedMemoryAllocator;
use hardware_interface_io::memory::{
    MemoryAccess, MemoryAddress, MemoryAlignment, SharedMemoryAccess,
};
use pdma_model_host::{ChannelConfig, RingConfig, Rings};

use crate::TestResult;

pub(super) struct Buffers {
    pub(super) config: ChannelConfig,
    pub(super) source: MemoryAddress,
    pub(super) destination: MemoryAddress,
    pub(super) length: usize,
    pub(super) value: u8,
    pub(super) fence: pdma_model_software::dma::CompletionWord,
}

impl Buffers {
    pub(super) fn allocate(allocator: &SharedMemoryAllocator, index: usize) -> TestResult<Self> {
        let command = allocator.allocate(
            8 * u64::try_from(Command::RECORD_SIZE)?,
            MemoryAlignment::try_new(u64::try_from(Command::RECORD_ALIGNMENT)?)?,
        )?;
        let response = allocator.allocate(
            u64::try_from(Event::RECORD_SIZE)?,
            MemoryAlignment::try_new(u64::try_from(Event::RECORD_ALIGNMENT)?)?,
        )?;
        let length = if index == 0 { 1024 } else { 16_384 };
        let source = allocator.allocate(u64::try_from(length)?, MemoryAlignment::ONE)?;
        let destination = allocator.allocate(2 * u64::try_from(length)?, MemoryAlignment::ONE)?;
        let value = 0x31 + u8::try_from(index)?;
        allocator
            .memory_access()
            .write(source, MemoryAlignment::ONE, &vec![value; length])?;
        let fence = allocator.allocate(8, MemoryAlignment::try_new(8)?)?;
        allocator
            .memory_access()
            .write(fence, MemoryAlignment::ONE, &0_u64.to_le_bytes())?;
        Ok(Self {
            config: ChannelConfig {
                id: ChannelId::new(index),
                rings: Rings {
                    command: RingConfig::new(command.raw(), 8)?,
                    event: RingConfig::new(response.raw(), 1)?,
                },
            },
            source,
            destination,
            length,
            value,
            fence: pdma_model_software::dma::CompletionWord(pdma_model_contract::dma::WordWrite {
                target: pdma_model_contract::dma::WordTarget {
                    address: fence.raw(),
                    context: 0,
                    width: pdma_model_contract::dma::WordTargetWidth::Bits64,
                },
                value: 0,
            }),
        })
    }

    pub(super) fn command(&self, id: u64, second: bool) -> TestResult<Command> {
        let length = u64::try_from(self.length)?;
        let destination = self.destination.raw() + if second { length } else { 0 };
        Ok(Command::Copy(pdma_model_contract::dma::CopyCommand {
            id,
            copy_length: length,
            sg_context: 0,
            source: CopyCommandSourceValues::Linear(LinearEndpoint {
                address: self.source.raw(),
                offset: 0,
            }),
            destination: CopyCommandDestinationValues::Linear(LinearEndpoint {
                address: destination,
                offset: 0,
            }),
            ..pdma_model_contract::dma::CopyCommand::ZERO
        }))
    }

    pub(super) fn byte_matches(
        &self,
        memory: &SharedMemoryAccess<(), MemoryAccessError>,
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

    pub(super) fn snapshot(
        &self,
        memory: &SharedMemoryAccess<(), MemoryAccessError>,
    ) -> TestResult<Vec<u8>> {
        let mut bytes = vec![0; self.length * 2];
        memory.read(self.destination, MemoryAlignment::ONE, &mut bytes)?;
        Ok(bytes)
    }
}
