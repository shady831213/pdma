use super::{Input, Ports, TestResult, configure, work};
use hardware_interface_runtime::{
    AtomicReadOrder, AtomicWriteOrder, MemoryAlignment, RegisterAccess, SharedMemoryAllocator,
};
use pdma_model_contract::{
    control::{ControlResponse, ControlResponseStatus},
    dma::{
        ChannelBlock, ChannelId, Command, CommandRegisters, Response, SignalCommand,
        SignalOperation, SignalOrder, SignalTarget, WaitCommand, WaitCondition, WaitOrder,
    },
    memory::{ContextAttributes, FunctionId},
};
use pdma_model_software::signal::Signal;

/// Correlated recovery responses and post-reset signal observations.
#[derive(Debug)]
pub struct Outcome {
    pub(crate) channel: ChannelId,
    pub(crate) requests: [u64; 3],
    pub(crate) probe: ControlResponse,
    pub(crate) drain: ControlResponse,
    pub(crate) reset: ControlResponse,
    pub(crate) wait_pending: bool,
    pub(crate) before_fresh: i64,
    pub(crate) fresh: Response,
    pub(crate) final_value: i64,
}

pub(in crate::harness) async fn run(
    ports: Ports<'_>,
    pasid: SharedMemoryAllocator,
    input: Input,
) -> TestResult<Outcome> {
    let function = FunctionId::new(0);
    let (mut management, contexts) = configure(&ports, function).await?;
    let channel = management.acquire_channel(function).await?;
    let address = pasid.allocate(8, MemoryAlignment::try_new(8)?)?;
    let memory = pasid.memory_access();
    let signal = Signal::new(&memory, address);
    signal.store(input.initial, AtomicWriteOrder::Release)?;
    let target = SignalTarget {
        address: address.raw(),
        context: contexts[1].raw(),
    };
    let mut driver = work::driver(&ports, channel, contexts[0]).await?;
    driver
        .submit(Command::wait(
            input.ids[0],
            WaitCommand {
                target,
                condition: WaitCondition::Equal,
                order: WaitOrder::Acquire,
                compare: u64::from_le_bytes(input.release.to_le_bytes()),
            },
        ))
        .await?;
    let update = SignalCommand {
        target,
        operation: SignalOperation::Add,
        order: SignalOrder::AcquireRelease,
        value: u64::from_le_bytes(input.increment.to_le_bytes()),
        expected: 0,
    };
    driver.submit(Command::signal(input.ids[1], update)).await?;
    let consumer = ports.bus.addresses().dma
        + u32::try_from(
            channel.register_offset(1).ok_or("channel layout")?
                + ChannelBlock::COMMAND.offset()
                + CommandRegisters::CONSUMER.offset(),
        )?;
    loop {
        let mut word = [0; 4];
        ports.bus.read(consumer, &mut word).await?;
        if u32::from_le_bytes(word) == 1 {
            break;
        }
        tokio::task::yield_now().await;
    }
    let wait_pending = driver.response().await?.is_none();
    let drain_id = management
        .submit_drain_context(function, contexts[1])
        .await?;
    let probe_id = management
        .submit_create_context(function, ContextAttributes::DEFAULT)
        .await?;
    let probe = management.next_response().await?;
    if probe.id != probe_id.raw() || probe.status != ControlResponseStatus::ContextBusy {
        return Err(format!("Drain did not remain pending behind Wait: {probe:?}").into());
    }
    let reset_id = management.submit_reset(function, channel).await?;
    let mut results = [
        management.next_response().await?,
        management.next_response().await?,
    ];
    results.sort_by_key(|response| response.id);
    let [drain, reset] = results;
    if drain.id != drain_id.raw()
        || drain.status != ControlResponseStatus::MaintenanceFailed
        || reset.id != reset_id.raw()
        || reset.status != ControlResponseStatus::Success
    {
        return Err(format!("Drain/Reset recovery failed: {drain:?}, {reset:?}").into());
    }
    drop(driver);
    let before_fresh = signal.load(AtomicReadOrder::Acquire)?;
    let mut driver = work::driver(&ports, channel, contexts[0]).await?;
    driver.submit(Command::signal(input.ids[2], update)).await?;
    let fresh = work::completed(&mut driver).await?;
    let final_value = signal.load(AtomicReadOrder::Acquire)?;
    drop(driver);
    management.release_channel(function, channel).await?;
    for context in contexts {
        management.submit_destroy_context(function, context).await?;
        let response = management.next_response().await?;
        if response.status != ControlResponseStatus::Success {
            return Err(format!("context destruction failed: {:?}", response.status).into());
        }
    }
    Ok(Outcome {
        channel,
        requests: [drain_id.raw(), probe_id.raw(), reset_id.raw()],
        probe,
        drain,
        reset,
        wait_pending,
        before_fresh,
        fresh,
        final_value,
    })
}
