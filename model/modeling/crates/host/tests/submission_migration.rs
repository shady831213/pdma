//! Submission continuity across modeled CPU changes.

use hardware_interface_runtime::{
    MemoryAddress, MemoryAlignment, RegisterAccess, SharedMemory, SharedMemoryAccess,
    SharedMemoryAllocator, SoftwareCpu,
};
use pdma_model_contract::{Command, CommandRegisters, Endpoint, Response};
use pdma_model_dma::Dma;
use pdma_model_host::{Channel, ChannelConfig, RingConfig, Rings, runtime::Model};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::Notify;

struct DelayedBus {
    model: Model<SharedMemoryAccess>,
    delay: Arc<AtomicBool>,
    paused: Arc<Notify>,
    resume: Arc<Notify>,
}

impl RegisterAccess for DelayedBus {
    type Error = pdma_model_dma::Error;
    async fn read(&self, offset: u32, response: &mut [u8]) -> Result<(), Self::Error> {
        self.model.read(offset, response).await
    }
    async fn write(&self, offset: u32, request: &[u8]) -> Result<(), Self::Error> {
        if self.delay.load(Ordering::SeqCst) {
            self.paused.notify_one();
            self.resume.notified().await;
        }
        self.model.write(offset, request).await
    }
}

#[tokio::test]
async fn suspended_submission_completes_on_another_cpu() -> Result<(), Box<dyn std::error::Error>> {
    // Given: a submission suspended just before publishing its producer cursor.
    let allocator =
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000));
    let rings = Rings {
        command: RingConfig::new(
            allocator
                .allocate(
                    u64::try_from(Command::RECORD_SIZE)?,
                    MemoryAlignment::try_new(u64::try_from(Command::RECORD_ALIGNMENT)?)?,
                )?
                .raw(),
            1,
        )?,
        response: RingConfig::new(
            allocator
                .allocate(
                    u64::try_from(Response::RECORD_SIZE)?,
                    MemoryAlignment::try_new(u64::try_from(Response::RECORD_ALIGNMENT)?)?,
                )?
                .raw(),
            1,
        )?,
    };
    let model = Model::new(Dma::new(allocator.memory_access(), 1)?);
    let delay = Arc::new(AtomicBool::new(false));
    let paused = Arc::new(Notify::new());
    let resume = Arc::new(Notify::new());
    let bus = DelayedBus {
        model: model.clone(),
        delay: Arc::clone(&delay),
        paused: Arc::clone(&paused),
        resume: Arc::clone(&resume),
    };
    let owner = SoftwareCpu::new();
    let other = SoftwareCpu::new();
    let mut channel = owner
        .run_main(Channel::setup(
            bus,
            allocator.memory_access(),
            ChannelConfig {
                id: pdma_model_contract::ChannelId::new(0),
                rings,
            },
        ))
        .await?;
    delay.store(true, Ordering::SeqCst);
    let submission = channel.submit(Command {
        flags: pdma_model_contract::CompletionFlagsValues::default(),
        id: 1,
        source: Endpoint::linear(0, 0),
        destination: Endpoint::linear(0, 0),
    });
    tokio::pin!(submission);
    owner
        .run_main(async {
            tokio::select! {
                biased;
                () = paused.notified() => {},
                result = &mut submission => assert!(result.is_err(), "submission must suspend"),
            }
        })
        .await;

    // When: another CPU resumes the already-pending future.
    resume.notify_one();
    let result = other.run_main(&mut submission).await;

    // Then: the same submission publishes its producer exactly once.
    result?;
    let mut producer = [0; 4];
    model
        .read(
            u32::try_from(
                (pdma_model_contract::ChannelBlock::ALIGNMENT
                    + pdma_model_contract::ChannelBlock::COMMAND.offset())
                    + CommandRegisters::PRODUCER.offset(),
            )?,
            &mut producer,
        )
        .await?;
    assert_eq!(u32::from_le_bytes(producer), 1);
    Ok(())
}
