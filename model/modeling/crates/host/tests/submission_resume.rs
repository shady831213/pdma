//! Retained Host publication through the shared software ring endpoint.

use pdma_model_contract::dma::{
    ChannelBlock, ChannelId, Command, CommandRegisters, CopyCommandDestinationValues,
    CopyCommandSourceValues, Event, LinearEndpoint,
};

use hardware_interface_runtime::{
    MemoryAddress, MemoryAlignment, RegisterAccess, SharedMemory, SharedMemoryAccess,
    SharedMemoryAllocator, SoftwareCpu,
};
use pdma_model_dma::Dma;
use pdma_model_host::{
    Channel, ChannelConfig, DmaError as Error, RingConfig, Rings, runtime::Model,
};
use std::{
    future::{Future, pending, poll_fn},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    task::Poll,
};

struct CommitBus {
    model: Model<SharedMemoryAccess>,
    pause: Arc<AtomicBool>,
    commits: Arc<AtomicUsize>,
}

impl RegisterAccess for CommitBus {
    type Error = pdma_model_dma::Error;
    async fn read(&self, offset: u32, response: &mut [u8]) -> Result<(), Self::Error> {
        self.model.read(offset, response).await
    }
    async fn write(&self, offset: u32, request: &[u8]) -> Result<(), Self::Error> {
        self.model.write(offset, request).await?;
        self.commits.fetch_add(1, Ordering::SeqCst);
        if self.pause.swap(false, Ordering::SeqCst) {
            pending::<()>().await;
        }
        Ok(())
    }
}

#[tokio::test]
async fn cancelled_committed_submission_resumes_without_republication()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: one software channel with real hardware-backed registers and ring memory.
    let allocator =
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000));
    let alignment = MemoryAlignment::try_new(8)?;
    let rings = Rings {
        command: RingConfig::new(
            allocator
                .allocate(u64::try_from(Command::RECORD_SIZE)?, alignment)?
                .raw(),
            1,
        )?,
        event: RingConfig::new(
            allocator
                .allocate(u64::try_from(Event::RECORD_SIZE)?, alignment)?
                .raw(),
            1,
        )?,
    };
    let model = Model::new(Dma::new(allocator.memory_access(), 1)?);
    let pause = Arc::new(AtomicBool::new(false));
    let commits = Arc::new(AtomicUsize::new(0));
    let bus = CommitBus {
        model: model.clone(),
        pause: Arc::clone(&pause),
        commits: Arc::clone(&commits),
    };
    let cpu = SoftwareCpu::new();
    let mut channel = cpu
        .run_main(Channel::setup(
            bus,
            allocator.memory_access(),
            ChannelConfig {
                id: ChannelId::new(0),
                rings,
            },
        ))
        .await?;
    let command = Command::Copy(pdma_model_contract::dma::CopyCommand {
        id: 41,
        source: CopyCommandSourceValues::Linear(LinearEndpoint {
            address: 0,
            offset: 0,
        }),
        destination: CopyCommandDestinationValues::Linear(LinearEndpoint {
            address: 0,
            offset: 0,
        }),
        copy_length: 0,
        sg_context: 0,
        ..pdma_model_contract::dma::CopyCommand::ZERO
    });
    pause.store(true, Ordering::SeqCst);
    commits.store(0, Ordering::SeqCst);
    // When: cancellation follows the committed producer write and software retries.
    {
        let mut submit = std::pin::pin!(channel.submit(command));
        cpu.run_main(poll_fn(|cx| {
            assert!(submit.as_mut().poll(cx).is_pending());
            Poll::Ready(())
        }))
        .await;
    }
    assert!(channel.has_pending_submission());
    assert!(matches!(
        cpu.run_main(channel.submit(command)).await,
        Err(Error::RingBusy)
    ));
    cpu.run_main(channel.resume_submission()).await?;
    // Then: the original entry has one publication and no retained submission remains.
    assert!(!channel.has_pending_submission());
    let offset = u32::try_from(
        pdma_model_contract::dma::ChannelBlock::ALIGNMENT
            + ChannelBlock::COMMAND.offset()
            + CommandRegisters::PRODUCER.offset(),
    )?;
    let mut bytes = [0; 4];
    model.read(offset, &mut bytes).await?;
    assert_eq!(u32::from_le_bytes(bytes), 1);
    assert_eq!(commits.load(Ordering::SeqCst), 1);
    Ok(())
}
