//! Submission continuity across modeled CPU changes.

use hw_model_io::memory::MemoryAccessError;
use pdma_model_contract::dma::{
    Command, CommandRegisters, CopyCommandDestinationValues, CopyCommandSourceValues, Event,
    LinearEndpoint,
};

use hw_model_io::memory::{MemoryAddress, MemoryAlignment, SharedMemoryAccess};
use hw_model_io::memory::{SharedMemory, SharedMemoryAllocator};
use hw_model_io::register::RegisterAccess;
use hw_model_swenv::SoftwareEnv;
use pdma_model_dma::Dma;
use pdma_model_host::{Channel, ChannelConfig, RingConfig, Rings, runtime::Model};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use tokio::sync::Notify;

struct DelayedBus {
    model: Model<SharedMemoryAccess<(), MemoryAccessError>>,
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
    let allocator = SharedMemoryAllocator::new(
        Arc::new(SharedMemory::new()),
        MemoryAddress::new(0x1000),
        64 * 1024,
    )
    .expect("valid test heap");
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
        event: RingConfig::new(
            allocator
                .allocate(
                    u64::try_from(Event::RECORD_SIZE)?,
                    MemoryAlignment::try_new(u64::try_from(Event::RECORD_ALIGNMENT)?)?,
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
    let owner = SoftwareEnv::new();
    let other = SoftwareEnv::new();
    let mut channel = owner
        .run_main(Channel::setup(
            bus,
            allocator.memory_access(),
            ChannelConfig {
                id: pdma_model_contract::dma::ChannelId::new(0),
                rings,
            },
        ))
        .await?;
    delay.store(true, Ordering::SeqCst);
    let submission = channel.submit(Command::Copy(pdma_model_contract::dma::CopyCommand {
        id: 1,
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
    }));
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
                (pdma_model_contract::dma::ChannelBlock::ALIGNMENT
                    + pdma_model_contract::dma::ChannelBlock::COMMAND.offset())
                    + CommandRegisters::PRODUCER.offset(),
            )?,
            &mut producer,
        )
        .await?;
    assert_eq!(u32::from_le_bytes(producer), 1);
    Ok(())
}
