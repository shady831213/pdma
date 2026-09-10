//! Host decoding rejects unknown channel fault encodings.

use hardware_interface_runtime::{
    MemoryAddress, RegisterAccess, SharedMemory, SharedMemoryAccess, SharedMemoryAllocator,
    SoftwareCpu,
};
use hardware_interface_schema::RegisterCodecError;
use pdma_model_contract::{ChannelRegisters, DeviceRegisters};
use pdma_model_device::Device;
use pdma_model_host::{Channel, Error, Model, RingConfig, Rings};
use std::sync::Arc;

struct CorruptFaultBus(Model<SharedMemoryAccess>);

impl RegisterAccess for CorruptFaultBus {
    type Error = pdma_model_device::Error;

    async fn read(&self, offset: u32, response: &mut [u8]) -> Result<(), Self::Error> {
        self.0.read(offset, response).await?;
        if usize::try_from(offset).ok()
            == Some(DeviceRegisters::CHANNEL.offset() + ChannelRegisters::FAULT.offset())
        {
            response.copy_from_slice(&3_u32.to_le_bytes());
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
    let allocator =
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000));
    let memory = allocator.memory_access();
    let bus = CorruptFaultBus(Model::new(Device::new(memory.clone())));
    let rings = Rings {
        command: RingConfig::new(0x1000, 1)?,
        response: RingConfig::new(0x2000, 1)?,
    };
    let cpu = SoftwareCpu::new();
    cpu.run_main(async {
        let channel = Channel::setup(bus, memory, rings).await?;
        // When: software reads the fault as a typed state.
        let result = channel.fault().await;
        // Then: unknown is reported explicitly rather than returned as a normal state.
        assert!(matches!(
            result,
            Err(Error::RegisterCodec(RegisterCodecError::UnknownTag {
                observed: 3,
                ..
            }))
        ));
        Ok::<_, Box<dyn std::error::Error>>(())
    })
    .await
}
