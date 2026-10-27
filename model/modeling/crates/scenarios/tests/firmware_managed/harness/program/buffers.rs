use hardware_interface_io::memory::MemoryAccessError;
use pdma_model_contract::dma::{
    ChannelId, Command, CopyCommandDestinationValues, CopyCommandSourceValues, Event,
    LinearEndpoint,
};

use super::{Ports, TestResult};
use crate::harness::runtime::HostBus;
use hardware_interface_io::memory::{
    MemoryAccess, MemoryAddress, MemoryAlignment, SharedMemoryAccess,
};
use pdma_model_contract::control::{ControlQueue, ControlRequest, ControlResponse};
use pdma_model_host::{
    Channel, ChannelConfig, Management, ManagementConfig, ManagementRings, RingConfig, Rings,
};
use pdma_model_software::control::InterruptEvent;

pub(super) type Driver<M = SharedMemoryAccess<(), MemoryAccessError>> =
    Channel<HostBus<M>, SharedMemoryAccess<(), MemoryAccessError>>;

pub(super) struct Transfer {
    pub(super) driver: Driver,
    command: Command,
    destination: MemoryAddress,
    length: usize,
    completion: pdma_model_software::dma::CompletionWord,
    memory: SharedMemoryAccess<(), MemoryAccessError>,
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
            u64::try_from(2 * Event::RECORD_SIZE)?,
            MemoryAlignment::try_new(u64::try_from(Event::RECORD_ALIGNMENT)?)?,
        )?;
        let length = u64::try_from(payload.len())?;
        let source = ports.allocator.allocate(length, MemoryAlignment::ONE)?;
        let destination = ports.allocator.allocate(length, MemoryAlignment::ONE)?;
        ports.memory.write(source, MemoryAlignment::ONE, payload)?;
        ports
            .memory
            .write(destination, MemoryAlignment::ONE, &vec![0; payload.len()])?;
        let fence = ports.allocator.allocate(8, MemoryAlignment::try_new(8)?)?;
        ports
            .memory
            .write(fence, MemoryAlignment::ONE, &0_u64.to_le_bytes())?;
        let driver = Channel::setup_at(
            ports.bus.clone(),
            ports.memory.clone(),
            ChannelConfig {
                id: channel,
                rings: Rings {
                    command: RingConfig::new(command.raw(), 2)?,
                    event: RingConfig::new(response.raw(), 2)?,
                },
            },
            ports.bus.addresses().dma,
        )
        .await?;
        driver.set_irq_enabled(false).await?;
        Ok(Self {
            driver,
            command: Command::Copy(pdma_model_contract::dma::CopyCommand {
                id: 0,
                copy_length: length,
                sg_context: 0,
                source: CopyCommandSourceValues::Linear(LinearEndpoint {
                    address: source.raw(),
                    offset: 0,
                }),
                destination: CopyCommandDestinationValues::Linear(LinearEndpoint {
                    address: destination.raw(),
                    offset: 0,
                }),
                ..pdma_model_contract::dma::CopyCommand::ZERO
            }),
            destination,
            length: payload.len(),
            memory: ports.memory.clone(),
            completion: pdma_model_software::dma::CompletionWord(
                pdma_model_contract::dma::WordWrite {
                    target: pdma_model_contract::dma::WordTarget {
                        address: fence.raw(),
                        context: 0,
                        width: pdma_model_contract::dma::WordTargetWidth::Bits64,
                    },
                    value: 0,
                },
            ),
        })
    }

    pub(super) async fn start(&mut self, id: u64) -> TestResult {
        let Command::Copy(copy) = &mut self.command else {
            return Err("expected Copy".into());
        };
        copy.id = id;
        self.completion.0.value = id;
        self.driver.submit(self.command).await?;
        self.driver.submit(self.completion.command(id)).await?;
        Ok(())
    }

    pub(super) fn bytes(
        &self,
        memory: &SharedMemoryAccess<(), MemoryAccessError>,
    ) -> TestResult<Vec<u8>> {
        read(memory, self.destination, self.length)
    }

    pub(super) fn retire(self) -> MemoryAddress {
        self.destination
    }

    pub(super) async fn wait_started(
        &self,
        memory: &SharedMemoryAccess<(), MemoryAccessError>,
    ) -> TestResult<usize> {
        loop {
            let copied = copied_prefix(&self.bytes(memory)?);
            if copied > 0 {
                return Ok(copied);
            }
            tokio::task::yield_now().await;
        }
    }

    pub(super) async fn completed(&mut self) -> TestResult<u64> {
        loop {
            if let Some(value) = self.try_completed().await? {
                return Ok(value);
            }
            tokio::task::yield_now().await;
        }
    }

    pub(super) async fn try_completed(&mut self) -> TestResult<Option<u64>> {
        if let Some(event) = self.driver.event().await? {
            return Err(format!("DMA error: {event:?}").into());
        }
        if self.driver.fatal().await? != pdma_model_contract::dma::ChannelFatalCode::None {
            return Err("DMA fatal".into());
        }
        Ok(self
            .completion
            .is_complete(&self.memory)?
            .then_some(self.completion.0.value))
    }
}

pub(super) fn copied_prefix(bytes: &[u8]) -> usize {
    bytes
        .iter()
        .position(|byte| *byte != 0x6d)
        .unwrap_or(bytes.len())
}

pub(super) fn read(
    memory: &SharedMemoryAccess<(), MemoryAccessError>,
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
) -> TestResult<
    Management<
        HostBus<SharedMemoryAccess<(), MemoryAccessError>>,
        SharedMemoryAccess<(), MemoryAccessError>,
    >,
> {
    management_with_context(
        ports,
        interrupt,
        pdma_model_contract::memory::ContextAttributes::DEFAULT,
    )
    .await
}

pub(in crate::harness::program) async fn management_with_context<M: Send>(
    ports: &Ports<'_, M>,
    interrupt: InterruptEvent,
    attributes: pdma_model_contract::memory::ContextAttributes,
) -> TestResult<Management<HostBus<M>, SharedMemoryAccess<(), MemoryAccessError>>> {
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
