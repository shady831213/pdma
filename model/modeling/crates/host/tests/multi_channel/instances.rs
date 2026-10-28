use super::{
    Arc, AtomicUsize, COUNT, Channel, ChannelId, Dma, Error, Model, Ordering, RegisterAccess,
    SharedMemoryAccess, SoftwareEnv, TestResult, allocator, config,
};
use hw_model_io::memory::MemoryAccessError;

struct CountWrites {
    model: Model<SharedMemoryAccess<(), MemoryAccessError>>,
    writes: Arc<AtomicUsize>,
}

impl RegisterAccess for CountWrites {
    type Error = pdma_model_dma::Error;

    async fn read(&self, offset: u32, response: &mut [u8]) -> Result<(), Self::Error> {
        self.model.read(offset, response).await
    }

    async fn write(&self, offset: u32, request: &[u8]) -> Result<(), Self::Error> {
        self.writes.fetch_add(1, Ordering::SeqCst);
        self.model.write(offset, request).await
    }
}

#[tokio::test]
async fn setup_uses_each_instances_discovered_channel_range() -> TestResult {
    // Given: one software CPU can reach independent one-channel and five-channel devices.
    let allocator = allocator()?;
    let memory = allocator.memory_access();
    let instances = [
        (1, Model::new(Dma::new(memory.clone(), 1)?)),
        (5, Model::new(Dma::new(memory.clone(), 5)?)),
        (1021, Model::new(Dma::new(memory.clone(), 1021)?)),
    ];
    let cpu = SoftwareEnv::new();
    cpu.run_main(async {
        for (count, model) in instances {
            // When: software discovers each instance and selects its last valid channel.
            let mut bytes = [0; 4];
            model.read(u32::try_from(COUNT)?, &mut bytes).await?;
            assert_eq!(u32::from_le_bytes(bytes), u32::try_from(count)?);
            let mut channel = Channel::setup(
                model.clone(),
                memory.clone(),
                config(&allocator, count - 1)?,
            )
            .await?;
            // Then: the highest channel is usable, and invalid selections write no registers.
            assert_eq!(channel.event().await?, None);
            for index in [count, usize::MAX] {
                let writes = Arc::new(AtomicUsize::new(0));
                let bus = CountWrites {
                    model: model.clone(),
                    writes: Arc::clone(&writes),
                };
                let result = Channel::setup(bus, memory.clone(), config(&allocator, index)?).await;
                assert!(matches!(
                    result,
                    Err(Error::Channel { requested, available })
                        if requested == ChannelId::new(index)
                            && available == u32::try_from(count)?
                ));
                assert_eq!(writes.load(Ordering::SeqCst), 0);
            }
        }
        Ok::<_, Box<dyn std::error::Error>>(())
    })
    .await
}

#[tokio::test]
async fn instance_channel_counts_survive_channel_and_device_resets() -> TestResult {
    // Given: differently sized devices are both visible to one software program.
    let allocator = allocator()?;
    let memory = allocator.memory_access();
    let instances = [
        (1, Model::new(Dma::new(memory.clone(), 1)?)),
        (5, Model::new(Dma::new(memory.clone(), 5)?)),
    ];
    let cpu = SoftwareEnv::new();
    for (count, model) in instances {
        cpu.run_main(async {
            let config = config(&allocator, count - 1)?;
            let channel = Channel::setup(model.clone(), memory.clone(), config).await?;
            // When: software resets the highest channel and then the entire instance.
            channel.reset().await?;
            let mut bytes = [0; 4];
            model.read(u32::try_from(COUNT)?, &mut bytes).await?;
            assert_eq!(u32::from_le_bytes(bytes), u32::try_from(count)?);
            let channel = Channel::setup(model.clone(), memory.clone(), config).await?;
            channel.reset_dma().await?;
            // Then: reset retains discovery and permits fresh setup of the highest channel.
            model.read(u32::try_from(COUNT)?, &mut bytes).await?;
            assert_eq!(u32::from_le_bytes(bytes), u32::try_from(count)?);
            let mut channel = Channel::setup(model.clone(), memory.clone(), config).await?;
            assert_eq!(channel.event().await?, None);
            Ok::<_, Box<dyn std::error::Error>>(())
        })
        .await?;
    }
    Ok(())
}
