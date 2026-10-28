use pdma_model_contract::dma::{
    Command, CopyCommandDestinationValues, CopyCommandSourceValues, Event, LinearEndpoint,
};

const PF: FunctionId = FunctionId::new(0);

use super::*;
use hw_model_io::memory::MemoryAccess;
use hw_model_io::memory::MemoryAccessError;
use pdma_model_contract::memory::FunctionId;
use pdma_model_host::{Channel, ChannelConfig, Rings};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

const LENGTH: usize = 65_536;

#[derive(Clone)]
struct ObservedMemory {
    memory: SharedMemoryAccess<(), MemoryAccessError>,
    destination: Arc<AtomicU64>,
    writes: Arc<AtomicUsize>,
}

impl MemoryAccess for ObservedMemory {
    type Error = MemoryAccessError;

    fn validate_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        length: u64,
    ) -> Result<(), MemoryAccessError> {
        self.memory.validate(address, alignment, length)
    }
    fn read_with_sideband(
        &self,
        (): &(),
        address: MemoryAddress,
        alignment: MemoryAlignment,
        bytes: &mut [u8],
    ) -> Result<(), MemoryAccessError> {
        self.memory.read(address, alignment, bytes)
    }
    fn write_with_sideband(
        &self,
        (): &(),
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
    reset_case(true).await;
}

#[tokio::test]
async fn reset_during_copy_drains_old_work_and_preserves_allocation() {
    reset_case(false).await;
}

async fn reset_case(release: bool) {
    // Given: a real shared DMA engine with observation of its old destination writes.
    let allocator = SharedMemoryAllocator::new(
        Arc::new(SharedMemory::new()),
        MemoryAddress::new(0x1000),
        256 * 1024,
    )
    .expect("valid test heap");
    let memory = allocator.memory_access();
    let observed = ObservedMemory {
        memory: memory.clone(),
        destination: Arc::new(AtomicU64::new(u64::MAX)),
        writes: Arc::new(AtomicUsize::new(0)),
    };
    let (system, host) = Managed::new(observed.clone()).unwrap();
    let program = Box::pin(client::run(host.runtime, async |interrupt| {
        let mut management = Management::setup(
            host.bus.clone(),
            memory.clone(),
            client::config(rings(&allocator).unwrap(), &interrupt),
        )
        .await
        .unwrap();
        let id = management.acquire_channel(PF).await.unwrap();
        let command = ring::<Command>(&allocator).unwrap();
        let response = ring::<Event>(&allocator).unwrap();
        let (source, destination) = prepare_copy(&allocator, &observed);
        let config = ChannelConfig {
            id,
            rings: Rings {
                command,
                event: response,
            },
        };
        let mut channel = Channel::setup(host.bus.clone(), memory.clone(), config)
            .await
            .unwrap();
        channel.set_irq_enabled(false).await.unwrap();
        channel
            .submit(copy(
                101,
                (source, destination),
                u64::try_from(LENGTH).unwrap(),
            ))
            .await
            .unwrap();
        while observed.writes.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
        // When: Host releases the channel after actual payload writes have started.
        drop(channel);
        if release {
            management.release_channel(PF, id).await.unwrap();
        } else {
            management.reset_channel(PF, id).await.unwrap();
        }
        let old_writes = observed.writes.load(Ordering::SeqCst);
        for _ in 0..64 {
            tokio::task::yield_now().await;
        }
        // Then: the release response is a drain boundary, and a fresh grant works.
        assert!(old_writes > 0 && old_writes < LENGTH / pdma_model_dma::COPY_CHUNK_BYTES);
        assert_eq!(observed.writes.load(Ordering::SeqCst), old_writes);
        if release {
            assert_eq!(management.acquire_channel(PF).await.unwrap(), id);
        } else {
            assert!(matches!(
                management.acquire_channel(PF).await,
                Err(Error::ManagementStatus(ControlResponseStatus::NoResources))
            ));
            let address = u32::try_from(
                id.register_offset(1).unwrap()
                    + pdma_model_contract::dma::ChannelBlock::CONTROL.offset()
                    + pdma_model_contract::dma::ChannelRegisters::CONFIG.offset()
                    + pdma_model_contract::dma::ChannelConfiguration::ENABLE.offset(),
            )
            .unwrap();
            let mut enabled = [0xff; 4];
            host.bus.read(address, &mut enabled).await.unwrap();
            assert_eq!(enabled, [0; 4]);
        }
        let destination = allocator.allocate(5, MemoryAlignment::ONE).unwrap();
        let mut channel = Channel::setup(host.bus.clone(), memory.clone(), config)
            .await
            .unwrap();
        channel.set_irq_enabled(false).await.unwrap();
        channel
            .submit(copy(202, (source, destination), 5))
            .await
            .unwrap();
        complete(&mut channel, &allocator).await;

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

fn prepare_copy(
    allocator: &SharedMemoryAllocator,
    observed: &ObservedMemory,
) -> (MemoryAddress, MemoryAddress) {
    let source = allocator
        .allocate(u64::try_from(LENGTH).unwrap(), MemoryAlignment::ONE)
        .unwrap();
    let destination = allocator
        .allocate(u64::try_from(LENGTH).unwrap(), MemoryAlignment::ONE)
        .unwrap();
    observed
        .destination
        .store(destination.raw(), Ordering::SeqCst);
    observed
        .memory
        .write(source, MemoryAlignment::ONE, &vec![0x6d; LENGTH])
        .unwrap();
    (source, destination)
}

fn copy(id: u64, (source, destination): (MemoryAddress, MemoryAddress), length: u64) -> Command {
    Command::Copy(pdma_model_contract::dma::CopyCommand {
        id,
        copy_length: length,
        sg_context: 0,
        source: CopyCommandSourceValues::Linear(LinearEndpoint {
            address: source.raw(),
            offset: 0,
        }),
        destination: CopyCommandDestinationValues::Linear(LinearEndpoint {
            address: destination.raw(),
            offset: 0,
        }),
        ..pdma_model_contract::dma::CopyCommand::ZERO
    })
}

async fn complete(
    channel: &mut Channel<
        super::super::super::HostBus<ObservedMemory>,
        SharedMemoryAccess<(), MemoryAccessError>,
    >,
    allocator: &SharedMemoryAllocator,
) {
    let address = allocator
        .allocate(8, MemoryAlignment::try_new(8).unwrap())
        .unwrap();
    let fence = pdma_model_software::dma::CompletionWord(pdma_model_contract::dma::WordWrite {
        target: pdma_model_contract::dma::WordTarget {
            address: address.raw(),
            context: 0,
            width: pdma_model_contract::dma::WordTargetWidth::Bits64,
        },
        value: 202,
    });
    channel.submit(fence.command(202)).await.unwrap();
    while !fence.is_complete(&allocator.memory_access()).unwrap() {
        assert_eq!(channel.event().await.unwrap(), None);
        assert_eq!(
            channel.fatal().await.unwrap(),
            pdma_model_contract::dma::ChannelFatalCode::None
        );
        tokio::task::yield_now().await;
    }
}
