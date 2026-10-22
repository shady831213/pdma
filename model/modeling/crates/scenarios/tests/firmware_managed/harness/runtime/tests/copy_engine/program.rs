mod commands;
pub(super) mod waiting;
use hardware_interface_runtime::{
    MemoryAccess, MemoryAlignment, RegisterAccess, SharedMemoryAllocator,
};
use pdma_model_contract::dma::*;
use pdma_model_host::{Channel, ChannelConfig, CompletionWord, RingConfig, Rings};

pub(super) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub(super) struct Ports<B> {
    pub bus: B,
    pub allocator: SharedMemoryAllocator,
    pub channels: [ChannelId; 2],
}

#[derive(Debug)]
pub(super) struct Outcome {
    pub word: u32,
    pub permits: u64,
    pub fill: [u8; 11],
    pub event: Option<Event>,
    pub fence: bool,
    pub trap: bool,
}

pub(super) async fn run<B>(
    ports: Ports<B>,
    (supported, semaphore_first): (bool, bool),
) -> TestResult<Outcome>
where
    B: RegisterAccess + Clone + Send + Sync,
    B::Error: std::error::Error + Send + Sync + 'static,
{
    let memory = ports.allocator.memory_access();
    let mut channels = setup(&ports).await?;
    let mut peer = channels.pop().ok_or("peer channel missing")?;
    let mut selected = channels.pop().ok_or("selected channel missing")?;
    let storage = ports.allocator.allocate(64, MemoryAlignment::try_new(8)?)?;
    memory.write(storage, MemoryAlignment::ONE, &[0; 64])?;
    let target = |offset, width| WordTarget {
        address: storage.raw() + offset,
        context: 0,
        width,
    };
    let dependency = target(0, WordTargetWidth::Bits64);
    let word = target(8, WordTargetWidth::Bits32);
    let permits = target(16, WordTargetWidth::Bits64);
    let fence = CompletionWord(WordWrite {
        target: target(24, WordTargetWidth::Bits64),
        value: 99,
    });
    for command in commands::batch(storage.raw(), semaphore_first) {
        selected.submit(command).await?;
    }
    assert!(!selected.faulted().await?);
    assert!(!fence.is_complete(&memory)?);
    peer.submit(Command::write(
        10,
        WordWrite {
            target: dependency,
            value: 1,
        },
    ))
    .await?;
    let event = loop {
        if let Some(event) = selected.event().await? {
            break Some(event);
        }
        if selected.fatal().await? != ChannelFatalCode::None {
            return Err("unexpected DMA fatal".into());
        }
        if selected.irq_cause().await?.trap() {
            break None;
        }
        tokio::task::yield_now().await;
    };
    let trap = selected.irq_cause().await?.trap();
    if supported {
        selected.acknowledge_trap().await?;
    }
    let mut word_bytes = [0; 4];
    memory.read(word.address.into(), MemoryAlignment::ONE, &mut word_bytes)?;
    let mut permit_bytes = [0; 8];
    memory.read(
        permits.address.into(),
        MemoryAlignment::ONE,
        &mut permit_bytes,
    )?;
    let mut fill = [0; 11];
    memory.read((storage.raw() + 33).into(), MemoryAlignment::ONE, &mut fill)?;
    Ok(Outcome {
        word: u32::from_le_bytes(word_bytes),
        permits: u64::from_le_bytes(permit_bytes),
        fill,
        event,
        fence: fence.is_complete(&memory)?,
        trap,
    })
}

pub(super) async fn setup<B>(
    ports: &Ports<B>,
) -> TestResult<Vec<Channel<B, hardware_interface_runtime::SharedMemoryAccess>>>
where
    B: RegisterAccess + Clone + Send + Sync,
    B::Error: std::error::Error + Send + Sync + 'static,
{
    let memory = ports.allocator.memory_access();
    let mut channels = Vec::new();
    for id in ports.channels {
        let command = ports.allocator.allocate(
            16 * u64::try_from(Command::RECORD_SIZE)?,
            MemoryAlignment::try_new(8)?,
        )?;
        let event = ports.allocator.allocate(
            2 * u64::try_from(Event::RECORD_SIZE)?,
            MemoryAlignment::try_new(8)?,
        )?;
        let driver = Channel::setup(
            ports.bus.clone(),
            memory.clone(),
            ChannelConfig {
                id,
                rings: Rings {
                    command: RingConfig::new(command.raw(), 16)?,
                    event: RingConfig::new(event.raw(), 2)?,
                },
            },
        )
        .await?;
        driver.set_irq_enabled(false).await?;
        channels.push(driver);
    }
    Ok(channels)
}
