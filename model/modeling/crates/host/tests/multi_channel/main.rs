//! Host channel discovery and independent reset boundaries.

mod instances;

use hardware_interface_pipeline::{
    AfterEach, AsyncExecutor, AsyncInteractionBackend, Operation, Repeat, StepOutcome, Then,
};
use hardware_interface_runtime::{
    CpuContextError, MemoryAddress, MemoryAlignment, RegisterAccess, SharedMemory,
    SharedMemoryAccess, SharedMemoryAllocator, SoftwareCpu,
};
use pdma_model_contract::{
    ChannelId, Command, CompletionFlagsValues, DeviceControl, DeviceRegisters, Endpoint, Response,
};
use pdma_model_device::{
    CheckLengths, Device, NextChunk, ReadCopy, ResetControl, Resettable, UpdateIrq, WriteCopy,
};
use pdma_model_host::{Channel, ChannelConfig, Error, RingConfig, Rings, runtime::Model};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
const COUNT: usize = DeviceRegisters::CONTROL.offset() + DeviceControl::CHANNEL_NUM.offset();

async fn with_idle_hardware<F: std::future::Future>(
    model: &Model<SharedMemoryAccess>,
    software: F,
) -> F::Output {
    let mut control = ResetControl;
    let mut operation = Operation::new(());
    tokio::pin!(software);
    loop {
        tokio::select! {
            result = &mut software => return result,
            result = model.interact(|device| {
                assert!(matches!(operation.step(&mut control, device), StepOutcome::Progressed));
                StepOutcome::<(), std::convert::Infallible, ()>::Progressed
            }) => {
                let Ok(outcome) = result;
                assert!(matches!(outcome, StepOutcome::Progressed));
            }
        }
    }
}

fn allocator() -> SharedMemoryAllocator {
    SharedMemoryAllocator::new(Arc::new(SharedMemory::new()), MemoryAddress::new(0x1000))
}

fn config(allocator: &SharedMemoryAllocator, index: usize) -> TestResult<ChannelConfig> {
    Ok(ChannelConfig {
        id: ChannelId::new(index),
        rings: Rings {
            command: RingConfig::new(
                allocator
                    .allocate(
                        u64::try_from(Command::RECORD_SIZE)?,
                        MemoryAlignment::try_new(8)?,
                    )?
                    .raw(),
                1,
            )?,
            response: RingConfig::new(
                allocator
                    .allocate(
                        u64::try_from(Response::RECORD_SIZE)?,
                        MemoryAlignment::try_new(8)?,
                    )?
                    .raw(),
                1,
            )?,
        },
    })
}

struct ObservedBus {
    model: Model<SharedMemoryAccess>,
    available: u32,
    reads: Arc<AtomicUsize>,
    writes: Arc<AtomicUsize>,
}

impl RegisterAccess for ObservedBus {
    type Error = pdma_model_device::Error;

    async fn read(&self, offset: u32, response: &mut [u8]) -> Result<(), Self::Error> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.model.read(offset, response).await?;
        if usize::try_from(offset).ok() == Some(COUNT) {
            response.copy_from_slice(&self.available.to_le_bytes());
        }
        Ok(())
    }

    async fn write(&self, offset: u32, request: &[u8]) -> Result<(), Self::Error> {
        self.writes.fetch_add(1, Ordering::SeqCst);
        self.model.write(offset, request).await
    }
}

#[tokio::test]
async fn discovery_rejects_unavailable_channels_before_any_configuration_write() -> TestResult {
    // Given: the bus advertises two channels despite a three-channel backing device.
    let allocator = allocator();
    let memory = allocator.memory_access();
    let model = Model::new(Device::new(memory.clone(), 3)?);
    let cpu = SoftwareCpu::new();
    for index in [2, 3, usize::MAX] {
        let writes = Arc::new(AtomicUsize::new(0));
        let bus = ObservedBus {
            model: model.clone(),
            available: 2,
            reads: Arc::new(AtomicUsize::new(0)),
            writes: Arc::clone(&writes),
        };
        // When: setup selects an unavailable or unrepresentable channel.
        let result = cpu
            .run_main(Channel::setup(
                bus,
                memory.clone(),
                config(&allocator, index)?,
            ))
            .await;
        // Then: discovery rejects it without mutating any register.
        assert!(
            matches!(result, Err(Error::Channel { requested, available: 2 }) if requested == ChannelId::new(index))
        );
        assert_eq!(writes.load(Ordering::SeqCst), 0);
    }
    Ok(())
}

#[tokio::test]
async fn setup_requires_a_cpu_before_channel_discovery() -> TestResult {
    // Given: a setup request outside a software CPU context.
    let allocator = allocator();
    let memory = allocator.memory_access();
    let reads = Arc::new(AtomicUsize::new(0));
    let writes = Arc::new(AtomicUsize::new(0));
    let bus = ObservedBus {
        model: Model::new(Device::new(memory.clone(), 3)?),
        available: 3,
        reads: Arc::clone(&reads),
        writes: Arc::clone(&writes),
    };
    // When: software tries to construct a channel driver.
    let result = Channel::setup(bus, memory, config(&allocator, 0)?).await;
    // Then: CPU rejection precedes both discovery and configuration.
    assert!(matches!(result, Err(Error::Cpu(CpuContextError::Inactive))));
    assert_eq!(reads.load(Ordering::SeqCst), 0);
    assert_eq!(writes.load(Ordering::SeqCst), 0);
    Ok(())
}

#[tokio::test]
async fn resetting_channel_zero_preserves_other_channels_pending_work() -> TestResult {
    // Given: three configured channels with independent ring storage and queued commands.
    let allocator = allocator();
    let memory = allocator.memory_access();
    let device = Device::new(memory.clone(), 3)?;
    let chunk = Then::new(Then::new(NextChunk, ReadCopy), WriteCopy);
    let execution = Then::new(CheckLengths, Repeat::new(chunk));
    let mut hardware = Then::new(
        Then::new(
            device.intake(),
            Resettable::new(AfterEach::new(execution, UpdateIrq)),
        ),
        device.responses(),
    );
    let model = Model::new(device);
    let cpu = SoftwareCpu::new();
    with_idle_hardware(
        &model,
        cpu.run_main(async {
            let mut channels = Vec::new();
            for index in 0..3 {
                let mut channel =
                    Channel::setup(model.clone(), memory.clone(), config(&allocator, index)?)
                        .await?;
                channel
                    .submit(Command {
                        flags: CompletionFlagsValues::default(),
                        id: u64::try_from(index)?,
                        source: Endpoint::linear(0, 0),
                        destination: Endpoint::linear(0, 0),
                    })
                    .await?;
                channels.push(channel);
            }
            // When: only channel zero is reset before hardware processes the pending work.
            channels.remove(0).reset().await?;
            // Then: the other two drivers retain their submissions and distinct completions.
            for (index, channel) in (1..3).zip(&mut channels) {
                AsyncExecutor::new(model.clone())
                    .run(&mut hardware, &mut Operation::new(()))
                    .await
                    .map_err(|error| format!("{error:?}"))?;
                assert_eq!(
                    channel.response().await?.map(|response| response.id),
                    Some(u64::try_from(index)?)
                );
                assert_eq!(channel.response().await?, None);
            }
            Ok::<_, Box<dyn std::error::Error>>(())
        }),
    )
    .await
}

#[tokio::test]
async fn channel_count_is_read_only_and_survives_both_reset_scopes() -> TestResult {
    // Given: software discovers a configured three-channel device.
    let allocator = allocator();
    let memory = allocator.memory_access();
    let model = Model::new(Device::new(memory.clone(), 3)?);
    let cpu = SoftwareCpu::new();
    let mut bytes = [0; 4];
    model.read(u32::try_from(COUNT)?, &mut bytes).await?;
    assert_eq!(u32::from_le_bytes(bytes), 3);
    let first = cpu
        .run_main(Channel::setup(
            model.clone(),
            memory.clone(),
            config(&allocator, 0)?,
        ))
        .await?;
    let second = cpu
        .run_main(Channel::setup(
            model.clone(),
            memory,
            config(&allocator, 1)?,
        ))
        .await?;
    // When: software attempts a count write and resets each scope.
    assert_eq!(
        model
            .write(u32::try_from(COUNT)?, &1_u32.to_le_bytes())
            .await,
        Err(pdma_model_device::Error::Access),
    );
    with_idle_hardware(&model, cpu.run_main(first.reset())).await?;
    model.read(u32::try_from(COUNT)?, &mut bytes).await?;
    assert_eq!(u32::from_le_bytes(bytes), 3);
    with_idle_hardware(&model, cpu.run_main(second.reset_device())).await?;
    // Then: immutable hardware discovery remains available after device reset.
    model.read(u32::try_from(COUNT)?, &mut bytes).await?;
    assert_eq!(u32::from_le_bytes(bytes), 3);
    Ok(())
}
