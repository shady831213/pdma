use super::{Grant, Input, Outcome, Ports, TestResult};
use crate::harness::program::buffers::Driver;
use hardware_interface_runtime::{
    AtomicReadOrder, AtomicWriteOrder, MemoryAlignment, SharedMemoryAllocator,
};
use pdma_model_contract::dma::{
    ChannelId, Command, ContextId, Response, SignalCommand, SignalOperation, SignalOrder,
    SignalTarget, WaitCommand, WaitCondition, WaitOrder,
};
use pdma_model_host::{Channel, ChannelConfig, RingConfig, Rings};
use pdma_model_software::{dma::ChannelControl, signal::Signal};
use tokio::sync::oneshot;

pub(in crate::harness) async fn run(
    ports: Ports<'_>,
    pasid: SharedMemoryAllocator,
    input: Input,
    granted: oneshot::Receiver<Grant>,
    finished: oneshot::Sender<()>,
) -> TestResult<Outcome> {
    let default_address = ports.allocator.allocate(8, MemoryAlignment::try_new(8)?)?;
    let address = pasid.allocate(8, MemoryAlignment::try_new(8)?)?;
    let memory = pasid.memory_access();
    let signal = Signal::new(&memory, address);
    let decoy = Signal::new(&ports.memory, default_address);
    signal.store(input.initial, AtomicWriteOrder::Release)?;
    decoy.store(input.decoys[2], AtomicWriteOrder::Release)?;
    let Grant { channels, contexts } = granted.await?;
    let mut waiting = driver(&ports, channels[0], contexts[0]).await?;
    let mut peer = driver(&ports, channels[1], contexts[0]).await?;
    let target = SignalTarget {
        address: address.raw(),
        context: contexts[1].raw(),
    };
    let [wait_id, successor_id, load_id, release_id] = input.ids;
    waiting
        .submit(Command::wait(
            wait_id,
            WaitCommand {
                target,
                condition: WaitCondition::Equal,
                order: WaitOrder::Acquire,
                compare: u64::from_le_bytes(input.release.to_le_bytes()),
            },
        ))
        .await?;
    waiting
        .submit(Command::signal(
            successor_id,
            SignalCommand {
                target,
                operation: SignalOperation::Add,
                order: SignalOrder::AcquireRelease,
                value: u64::from_le_bytes(input.increment.to_le_bytes()),
                expected: 0,
            },
        ))
        .await?;
    peer.submit(Command::signal(
        load_id,
        SignalCommand {
            target,
            operation: SignalOperation::Load,
            order: SignalOrder::Acquire,
            value: 0,
            expected: 0,
        },
    ))
    .await?;
    let loaded = completed(&mut peer).await?;
    let before_release = signal.load(AtomicReadOrder::Acquire)?;
    let successor_blocked = waiting.response().await?.is_none();
    peer.submit(Command::signal(
        release_id,
        SignalCommand {
            target,
            operation: SignalOperation::Store,
            order: SignalOrder::Release,
            value: u64::from_le_bytes(input.release.to_le_bytes()),
            expected: 0,
        },
    ))
    .await?;
    let released = completed(&mut peer).await?;
    let waited = completed(&mut waiting).await?;
    let successor = completed(&mut waiting).await?;
    let outcome = Outcome {
        responses: [loaded, released, waited, successor],
        before_release,
        successor_blocked,
        final_value: signal.load(AtomicReadOrder::Acquire)?,
        default_decoy: decoy.load(AtomicReadOrder::Acquire)?,
        same_address: default_address == address,
    };
    drop((waiting, peer));
    finished
        .send(())
        .map_err(|()| "PF stopped before channel release")?;
    Ok(outcome)
}

pub(super) async fn driver(
    ports: &Ports<'_>,
    channel: ChannelId,
    context: ContextId,
) -> TestResult<Driver> {
    let command = ports.allocator.allocate(
        u64::try_from(4 * Command::RECORD_SIZE)?,
        MemoryAlignment::try_new(8)?,
    )?;
    let response = ports.allocator.allocate(
        u64::try_from(4 * Response::RECORD_SIZE)?,
        MemoryAlignment::try_new(8)?,
    )?;
    ChannelControl::discover_at(&ports.bus, channel, ports.bus.addresses().dma)
        .await?
        .set_ring_context(&ports.bus, context)
        .await?;
    let driver = Channel::setup_at(
        ports.bus.clone(),
        ports.memory.clone(),
        ChannelConfig {
            id: channel,
            rings: Rings {
                command: RingConfig::new(command.raw(), 4)?,
                response: RingConfig::new(response.raw(), 4)?,
            },
        },
        ports.bus.addresses().dma,
    )
    .await?;
    driver.set_irq_enabled(false).await?;
    Ok(driver)
}

pub(super) async fn completed(driver: &mut Driver) -> TestResult<Response> {
    loop {
        if let Some(response) = driver.response().await? {
            return Ok(response);
        }
        tokio::task::yield_now().await;
    }
}
