use super::*;
use hardware_interface_runtime::{MemoryAccess, MemoryAccessError};
use pdma_model_contract::{Command, CompletionFlagsValues, Endpoint, Response, ResponseStatus};
use pdma_model_host::{Channel, ChannelConfig, Rings};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

const LENGTH: usize = 65_536;

#[derive(Clone)]
struct ObservedMemory {
    memory: SharedMemoryAccess,
    destination: Arc<AtomicU64>,
    writes: Arc<AtomicUsize>,
}

impl MemoryAccess for ObservedMemory {
    fn validate(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), MemoryAccessError> {
        self.memory.validate(address, alignment, length)
    }
    fn read(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &mut [u8],
    ) -> Result<(), MemoryAccessError> {
        self.memory.read(address, alignment, bytes)
    }
    fn write(
        &self,
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &[u8],
    ) -> Result<(), MemoryAccessError> {
        self.memory.write(address, alignment, bytes)?;
        let start = self.destination.load(Ordering::SeqCst);
        if address.raw() >= start && address.raw() < start.saturating_add(65_536) {
            self.writes.fetch_add(1, Ordering::SeqCst);
        }
        Ok(())
    }
}

#[tokio::test]
async fn release_during_copy_drains_old_work_before_response_and_reacquisition() {
    // Given: a real shared DMA engine with observation of its old destination writes.
    let allocator =
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000));
    let memory = allocator.memory_access();
    let observed = ObservedMemory {
        memory: memory.clone(),
        destination: Arc::new(AtomicU64::new(u64::MAX)),
        writes: Arc::new(AtomicUsize::new(0)),
    };
    let (system, host) = Managed::new(observed.clone()).unwrap();
    let cpu = SoftwareCpu::new();
    let program = Box::pin(cpu.run_main(async {
        let mut management =
            Management::setup(host.bus.clone(), memory.clone(), rings(&allocator).unwrap())
                .await
                .unwrap();
        let id = management.acquire_channel().await.unwrap();
        let command = ring::<Command>(&allocator).unwrap();
        let response = ring::<Response>(&allocator).unwrap();
        let source = allocator
            .allocate(u64::try_from(LENGTH).unwrap(), MemoryAlignment::ONE)
            .unwrap();
        let destination = allocator
            .allocate(u64::try_from(LENGTH).unwrap(), MemoryAlignment::ONE)
            .unwrap();
        observed
            .destination
            .store(destination.raw(), Ordering::SeqCst);
        memory
            .write(source, MemoryAlignment::ONE, &vec![0x6d; LENGTH])
            .unwrap();
        let config = ChannelConfig {
            id,
            rings: Rings { command, response },
        };
        let mut channel = Channel::setup(host.bus.clone(), memory.clone(), config)
            .await
            .unwrap();
        channel.set_irq_enabled(false).await.unwrap();
        channel
            .submit(Command {
                id: 101,
                source: Endpoint::linear(source.raw(), u64::try_from(LENGTH).unwrap()),
                destination: Endpoint::linear(destination.raw(), u64::try_from(LENGTH).unwrap()),
                flags: CompletionFlagsValues::default(),
            })
            .await
            .unwrap();
        while observed.writes.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
        // When: Host releases the channel after actual payload writes have started.
        drop(channel);
        management.release_channel(id).await.unwrap();
        let old_writes = observed.writes.load(Ordering::SeqCst);
        for _ in 0..64 {
            tokio::task::yield_now().await;
        }
        // Then: the release response is a drain boundary, and a fresh grant works.
        assert!(old_writes > 0 && old_writes < LENGTH / pdma_model_dma::COPY_CHUNK_BYTES);
        assert_eq!(observed.writes.load(Ordering::SeqCst), old_writes);
        assert_eq!(management.acquire_channel().await.unwrap(), id);
        let destination = allocator.allocate(5, MemoryAlignment::ONE).unwrap();
        let mut channel = Channel::setup(host.bus.clone(), memory.clone(), config)
            .await
            .unwrap();
        channel.set_irq_enabled(false).await.unwrap();
        channel
            .submit(Command {
                id: 202,
                source: Endpoint::linear(source.raw(), 5),
                destination: Endpoint::linear(destination.raw(), 5),
                flags: CompletionFlagsValues::default(),
            })
            .await
            .unwrap();
        let completion = loop {
            if let Some(response) = channel.response().await.unwrap() {
                break response;
            }
            tokio::task::yield_now().await;
        };
        assert_eq!(completion.id, 202);
        assert_eq!(completion.status, ResponseStatus::Success);
        let mut bytes = [0; 5];
        memory
            .read(destination, MemoryAlignment::ONE, &mut bytes)
            .unwrap();
        assert_eq!(bytes, [0x6d; 5]);
        assert_eq!(observed.writes.load(Ordering::SeqCst), old_writes);
    }));
    tokio::time::timeout(Duration::from_secs(3), system.run_until(program))
        .await
        .unwrap()
        .unwrap();
}
