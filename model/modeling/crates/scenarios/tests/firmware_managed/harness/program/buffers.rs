use super::{Ports, TestResult};
use crate::harness::runtime::HostBus;
use hardware_interface_runtime::{
    MemoryAccess, MemoryAddress, MemoryAlignment, SharedMemoryAccess,
};
use pdma_model_contract::control::{ControlQueue, ControlRequest, ControlResponse};
use pdma_model_contract::dma::{ChannelId, Command, CompletionFlagsValues, Endpoint, Response};
use pdma_model_host::{
    Channel, ChannelConfig, Management, ManagementConfig, ManagementRings, RingConfig, Rings,
};
use pdma_model_software::control::InterruptEvent;

pub(super) type Driver = Channel<HostBus<SharedMemoryAccess>, SharedMemoryAccess>;

pub(super) struct Transfer {
    pub(super) driver: Driver,
    command: Command,
    destination: MemoryAddress,
    length: usize,
}

impl Transfer {
    pub(super) async fn new(
        ports: &Ports<'_>,
        channel: ChannelId,
        payload: &[u8],
    ) -> TestResult<Self> {
        let command = ports.allocator.allocate(
            u64::try_from(2 * Command::RECORD_SIZE)?,
            MemoryAlignment::try_new(u64::try_from(Command::RECORD_ALIGNMENT)?)?,
        )?;
        let response = ports.allocator.allocate(
            u64::try_from(2 * Response::RECORD_SIZE)?,
            MemoryAlignment::try_new(u64::try_from(Response::RECORD_ALIGNMENT)?)?,
        )?;
        let length = u64::try_from(payload.len())?;
        let source = ports.allocator.allocate(length, MemoryAlignment::ONE)?;
        let destination = ports.allocator.allocate(length, MemoryAlignment::ONE)?;
        ports.memory.write(source, MemoryAlignment::ONE, payload)?;
        ports
            .memory
            .write(destination, MemoryAlignment::ONE, &vec![0; payload.len()])?;
        let driver = Channel::setup_at(
            ports.bus.clone(),
            ports.memory.clone(),
            ChannelConfig {
                id: channel,
                rings: Rings {
                    command: RingConfig::new(command.raw(), 2)?,
                    response: RingConfig::new(response.raw(), 2)?,
                },
            },
            ports.bus.addresses().dma,
        )
        .await?;
        driver.set_irq_enabled(false).await?;
        Ok(Self {
            driver,
            command: Command {
                sg_context: 0,
                id: 0,
                source: Endpoint::linear(source.raw(), length),
                destination: Endpoint::linear(destination.raw(), length),
                flags: CompletionFlagsValues::default(),
            },
            destination,
            length: payload.len(),
        })
    }

    pub(super) async fn start(&mut self, id: u64) -> TestResult {
        self.command.id = id;
        self.driver.submit(self.command).await?;
        Ok(())
    }

    pub(super) fn bytes(&self, memory: &SharedMemoryAccess) -> TestResult<Vec<u8>> {
        read(memory, self.destination, self.length)
    }

    pub(super) fn retire(self) -> MemoryAddress {
        self.destination
    }

    pub(super) async fn wait_started(&self, memory: &SharedMemoryAccess) -> TestResult<usize> {
        loop {
            let copied = copied_prefix(&self.bytes(memory)?);
            if copied > 0 {
                return Ok(copied);
            }
            tokio::task::yield_now().await;
        }
    }

    pub(super) async fn completed(&mut self) -> TestResult<Response> {
        loop {
            if let Some(response) = self.driver.response().await? {
                return Ok(response);
            }
            tokio::task::yield_now().await;
        }
    }
}

pub(super) fn copied_prefix(bytes: &[u8]) -> usize {
    bytes
        .iter()
        .position(|byte| *byte != 0x6d)
        .unwrap_or(bytes.len())
}

pub(super) fn read(
    memory: &SharedMemoryAccess,
    address: MemoryAddress,
    length: usize,
) -> TestResult<Vec<u8>> {
    let mut bytes = vec![0; length];
    memory.read(address, MemoryAlignment::ONE, &mut bytes)?;
    Ok(bytes)
}

pub(in crate::harness::program) async fn management(
    ports: &Ports<'_>,
    interrupt: InterruptEvent,
) -> TestResult<Management<HostBus<SharedMemoryAccess>, SharedMemoryAccess>> {
    management_with_context(
        ports,
        interrupt,
        pdma_model_contract::memory::ContextAttributes::DEFAULT,
    )
    .await
}

pub(in crate::harness::program) async fn management_with_context(
    ports: &Ports<'_>,
    interrupt: InterruptEvent,
    attributes: pdma_model_contract::memory::ContextAttributes,
) -> TestResult<Management<HostBus<SharedMemoryAccess>, SharedMemoryAccess>> {
    let event_interrupt = InterruptEvent::new();
    ports.interrupts.register(
        super::HostVector::Event,
        super::Handler::Control(event_interrupt.handler()),
    )?;
    let event = ports.allocator.allocate(
        8 * u64::try_from(pdma_model_contract::control::ControlEvent::RECORD_SIZE)?,
        MemoryAlignment::try_new(4)?,
    )?;
    let command = ports.allocator.allocate(
        u64::try_from(8 * ControlRequest::RECORD_SIZE)?,
        MemoryAlignment::try_new(u64::try_from(ControlRequest::RECORD_ALIGNMENT)?)?,
    )?;
    let response = ports.allocator.allocate(
        u64::try_from(8 * ControlResponse::RECORD_SIZE)?,
        MemoryAlignment::try_new(u64::try_from(ControlResponse::RECORD_ALIGNMENT)?)?,
    )?;
    let state = ports.allocator.allocate(
        u64::try_from(ControlQueue::RECORD_SIZE)?,
        MemoryAlignment::try_new(u64::try_from(ControlQueue::RECORD_ALIGNMENT)?)?,
    )?;
    Ok(Management::setup_with_context(
        ports.bus.clone(),
        ports.memory.clone(),
        (
            ManagementConfig {
                addresses: ports.bus.addresses(),
                interrupt,
                event_interrupt,
                rings: ManagementRings {
                    state,
                    command: RingConfig::new(command.raw(), 8)?,
                    response: RingConfig::new(response.raw(), 8)?,
                    event: RingConfig::new(event.raw(), 8)?,
                },
            },
            attributes,
        ),
    )
    .await?)
}
