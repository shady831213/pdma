//! Host decoding rejects unknown channel fault encodings.

use hw_model_io::memory::MemoryAccessError;
use hw_model_io::memory::{MemoryAddress, SharedMemoryAccess};
use hw_model_io::memory::{SharedMemory, SharedMemoryAllocator};
use hw_model_io::register::RegisterAccess;
use hw_model_schema::RegisterCodecError;
use hw_model_swenv::SoftwareEnv;
use pdma_model_contract::dma::ChannelRecovery;
use pdma_model_contract::dma::ChannelRegisters;
use pdma_model_dma::Dma;
use pdma_model_host::{
    Channel, ChannelConfig, DmaError as Error, RingConfig, Rings, runtime::Model,
};
use std::sync::Arc;

struct CorruptFaultBus(Model<SharedMemoryAccess<(), MemoryAccessError>>);

impl RegisterAccess for CorruptFaultBus {
    type Error = pdma_model_dma::Error;

    async fn read(&self, offset: u32, response: &mut [u8]) -> Result<(), Self::Error> {
        self.0.read(offset, response).await?;
        if usize::try_from(offset).ok()
            == Some(
                (pdma_model_contract::dma::ChannelBlock::ALIGNMENT
                    + pdma_model_contract::dma::ChannelBlock::CONTROL.offset())
                    + ChannelRegisters::RECOVERY.offset()
                    + ChannelRecovery::FATAL.offset(),
            )
        {
            response.copy_from_slice(&255_u32.to_le_bytes());
        }
        Ok(())
    }

    async fn write(&self, offset: u32, request: &[u8]) -> Result<(), Self::Error> {
        self.0.write(offset, request).await
    }
}

#[tokio::test]
async fn unknown_fault_is_a_codec_error() -> Result<(), Box<dyn std::error::Error>> {
    // Given: a real register transport with a corrupted fault read.
    let allocator = SharedMemoryAllocator::new(
        Arc::new(SharedMemory::new()),
        MemoryAddress::new(0x1000),
        64 * 1024,
    )
    .expect("valid test heap");
    let memory = allocator.memory_access();
    let bus = CorruptFaultBus(Model::new(Dma::new(memory.clone(), 1)?));
    let rings = Rings {
        command: RingConfig::new(0x1000, 1)?,
        event: RingConfig::new(0x2000, 1)?,
    };
    let cpu = SoftwareEnv::new();
    cpu.run_main(async {
        let channel = Channel::setup(
            bus,
            memory,
            ChannelConfig {
                id: pdma_model_contract::dma::ChannelId::new(0),
                rings,
            },
        )
        .await?;
        // When: software reads the fault as a typed state.
        let result = channel.fatal().await;
        // Then: unknown is reported explicitly rather than returned as a normal state.
        assert!(matches!(
            result,
            Err(Error::RegisterCodec(RegisterCodecError::UnknownTag {
                observed: 255,
                ..
            }))
        ));
        Ok::<_, Box<dyn std::error::Error>>(())
    })
    .await
}
