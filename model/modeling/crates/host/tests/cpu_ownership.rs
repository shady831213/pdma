//! CPU ownership across a suspended register transaction.

use hardware_interface_runtime::{
    CpuContextError, MemoryAddress, MemoryAlignment, RegisterAccess, SharedMemory,
    SharedMemoryAccess, SharedMemoryAllocator, SoftwareCpu,
};
use pdma_model_contract::{Command, CommandRegisters, DeviceRegisters};
use pdma_model_device::Device;
use pdma_model_host::{Channel, Error, Model, RingConfig, Rings};
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
    type Error = pdma_model_device::Error;
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
async fn suspended_submission_rejects_repoll_on_another_cpu()
-> Result<(), Box<dyn std::error::Error>> {
    // Given: a submission suspended just before publishing its producer cursor.
    let allocator =
        SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000));
    let alignment = MemoryAlignment::try_new(8)?;
    let rings = Rings {
        command: RingConfig::new(allocator.allocate(32, alignment)?.raw(), 1)?,
        response: RingConfig::new(allocator.allocate(16, alignment)?.raw(), 1)?,
    };
    let model = Model::new(Device::new(allocator.memory_access()));
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
        .run_main(Channel::setup(bus, allocator.memory_access(), rings))
        .await?;
    delay.store(true, Ordering::SeqCst);
    let submission = channel.submit(Command {
        id: 1,
        source: 0,
        destination: 0,
        length: 0,
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

    // Then: CPU ownership rejects it before the register commit.
    assert!(matches!(result, Err(Error::Cpu(CpuContextError::Mismatch))));
    let mut producer = [0; 4];
    model
        .read(
            u32::try_from(DeviceRegisters::COMMAND.offset() + CommandRegisters::PRODUCER.offset())?,
            &mut producer,
        )
        .await?;
    assert_eq!(u32::from_le_bytes(producer), 0);
    Ok(())
}
