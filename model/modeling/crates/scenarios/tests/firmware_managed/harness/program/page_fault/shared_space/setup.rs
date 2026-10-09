use super::*;
use pdma_model_host::{Channel, ChannelConfig, RingConfig, Rings};
use pdma_model_software::dma::ChannelControl;

pub(super) async fn driver<M: Send>(
    ports: &Ports<'_, M>,
    channel: ChannelId,
    ring_context: ContextId,
) -> TestResult<Driver<M>> {
    let commands = ports.allocator.allocate(
        u64::try_from(2 * Command::RECORD_SIZE)?,
        MemoryAlignment::try_new(u64::try_from(Command::RECORD_ALIGNMENT)?)?,
    )?;
    let responses = ports.allocator.allocate(
        u64::try_from(2 * Response::RECORD_SIZE)?,
        MemoryAlignment::try_new(u64::try_from(Response::RECORD_ALIGNMENT)?)?,
    )?;
    ChannelControl::discover_at(&ports.bus, channel, ports.bus.addresses().dma)
        .await?
        .set_ring_context(&ports.bus, ring_context)
        .await?;
    let driver = Channel::setup_at(
        ports.bus.clone(),
        ports.memory.clone(),
        ChannelConfig {
            id: channel,
            rings: Rings {
                command: RingConfig::new(commands.raw(), 2)?,
                response: RingConfig::new(responses.raw(), 2)?,
            },
        },
        ports.bus.addresses().dma,
    )
    .await?;
    driver.set_irq_enabled(false).await?;
    Ok(driver)
}

pub(super) async fn parked<M: Send>(driver: &mut Driver<M>) -> TestResult {
    while !driver.faulted().await? {
        tokio::task::yield_now().await;
    }
    if driver.response().await?.is_some() {
        return Err("parked channel published a response".into());
    }
    Ok(())
}

pub(super) async fn completed<M: Send>(driver: &mut Driver<M>) -> TestResult<Response> {
    loop {
        if let Some(response) = driver.response().await? {
            if driver.response().await?.is_some() {
                return Err("duplicate response".into());
            }
            return Ok(response);
        }
        tokio::task::yield_now().await;
    }
}

pub(super) struct Session<M> {
    pub manager: pdma_model_host::Management<
        crate::harness::runtime::HostBus<M>,
        hardware_interface_runtime::SharedMemoryAccess,
    >,
    pub channels: [ChannelId; 2],
    pub ring_context: ContextId,
    pub payload_context: ContextId,
}

impl<M: Send> Session<M> {
    pub(super) async fn recycle(
        &mut self,
        lifecycle: Lifecycle,
        channel: ChannelId,
    ) -> TestResult<ChannelId> {
        let function = FunctionId::new(0);
        match lifecycle {
            Lifecycle::Reset => {
                self.manager.reset_channel(function, channel).await?;
                Ok(channel)
            }
            Lifecycle::Release => {
                self.manager.release_channel(function, channel).await?;
                Ok(self.manager.acquire_channel(function).await?)
            }
        }
    }
}

pub(super) async fn session<M: Send>(ports: &Ports<'_, M>) -> TestResult<Session<M>> {
    pdma_model_host::firmware::wait_ready(
        &ports.bus,
        ports.bus.addresses().firmware,
        std::time::Duration::from_secs(1),
    )
    .await?;
    let irq = InterruptEvent::new();
    ports
        .interrupts
        .register(HostVector::Control, Handler::Control(irq.handler()))?;
    let mut manager =
        buffers::management_with_context(ports, irq, ContextAttributes::DEFAULT).await?;
    let function = FunctionId::new(0);
    let mut contexts = Vec::new();
    for attributes in [ContextAttributes::DEFAULT, ATTRIBUTES] {
        manager.submit_create_context(function, attributes).await?;
        let response = manager.next_response().await?;
        if response.status != ControlResponseStatus::Success {
            return Err("create context failed".into());
        }
        contexts.push(ContextId::new(response.context));
    }
    let [ring_context, payload_context]: [ContextId; 2] =
        contexts.try_into().map_err(|_| "context count")?;
    let channels = [
        manager.acquire_channel(function).await?,
        manager.acquire_channel(function).await?,
    ];
    Ok(Session {
        manager,
        channels,
        ring_context,
        payload_context,
    })
}

pub(super) fn requests(
    payload: &Space,
    context: ContextId,
) -> TestResult<(Command, [hardware_interface_runtime::MemoryAddress; 2])> {
    let source = payload
        .allocator
        .allocate(256, MemoryAlignment::try_new(256)?)?;
    let destinations = [
        payload.allocator.allocate(256, MemoryAlignment::ONE)?,
        payload.allocator.allocate(256, MemoryAlignment::ONE)?,
    ];
    payload
        .allocator
        .memory_access()
        .write(source, MemoryAlignment::ONE, &[0x6d; 256])?;
    payload.pages.evict(source);
    Ok((
        Command {
            source: Endpoint {
                context: context.raw(),
                ..Endpoint::linear(source.raw(), 0)
            },
            destination: Endpoint {
                context: context.raw(),
                ..Endpoint::linear(0, 0)
            },
            copy_length: 256,
            ..Command::ZERO
        },
        destinations,
    ))
}
