use super::{Ports, Spaces, TestResult};
use crate::harness::program::buffers::Driver;
use hardware_interface_runtime::{
    MemoryAccess, MemoryAddress, MemoryAlignment, SharedMemoryAccess,
};
use pdma_model_contract::dma::{ChannelId, Command, ContextId, Endpoint, Event, Segment};
use pdma_model_host::{Channel, ChannelConfig, RingConfig, Rings};
use pdma_model_software::dma::ChannelControl;

mod mixed;

pub(super) const PAYLOAD: &[u8] = b"PASID-backed scatter gather copy";

pub(super) struct Transfer {
    driver: Driver,
    command: Command,
    destination: MemoryAddress,
    fence: pdma_model_software::dma::CompletionWord,
    memory: SharedMemoryAccess,
    pub same_address: bool,
}

impl Transfer {
    pub(super) async fn new(
        ports: &Ports<'_>,
        (spaces, channel): (&Spaces, ChannelId),
        contexts: [ContextId; 4],
    ) -> TestResult<Self> {
        let length = u64::try_from(PAYLOAD.len())?;
        let source = spaces.source.allocate(length, MemoryAlignment::ONE)?;
        let destination = spaces.destination.allocate(length, MemoryAlignment::ONE)?;
        spaces
            .source
            .memory_access()
            .write(source, MemoryAlignment::ONE, PAYLOAD)?;
        spaces.destination.memory_access().write(
            destination,
            MemoryAlignment::ONE,
            &vec![0; PAYLOAD.len()],
        )?;
        let list = spaces.lists.allocate(
            u64::try_from(Segment::RECORD_SIZE)?,
            MemoryAlignment::try_new(8)?,
        )?;
        spaces.lists.memory_access().write_typed(
            list,
            MemoryAlignment::try_new(8)?,
            &Segment {
                address: source.raw(),
                length,
            }
            .to_bytes()?,
        )?;
        let command = ports.allocator.allocate(
            u64::try_from(8 * Command::RECORD_SIZE)?,
            MemoryAlignment::try_new(8)?,
        )?;
        let response = ports.allocator.allocate(
            u64::try_from(4 * Event::RECORD_SIZE)?,
            MemoryAlignment::try_new(8)?,
        )?;
        ChannelControl::discover_at(&ports.bus, channel, ports.bus.addresses().dma)
            .await?
            .set_ring_context(&ports.bus, contexts[1])
            .await?;
        let driver = Channel::setup_at(
            ports.bus.clone(),
            ports.memory.clone(),
            ChannelConfig {
                id: channel,
                rings: Rings {
                    command: RingConfig::new(command.raw(), 8)?,
                    event: RingConfig::new(response.raw(), 4)?,
                },
            },
            ports.bus.addresses().dma,
        )
        .await?;
        driver.set_irq_enabled(false).await?;
        let mut source_endpoint = Endpoint::scatter_gather(list.raw(), 1);
        source_endpoint.context = contexts[2].raw();
        let mut destination_endpoint = Endpoint::linear(destination.raw(), 0);
        destination_endpoint.context = contexts[3].raw();
        let address = ports.allocator.allocate(8, MemoryAlignment::try_new(8)?)?;
        ports
            .memory
            .write(address, MemoryAlignment::ONE, &0_u64.to_le_bytes())?;
        Ok(Self {
            driver,
            command: Command::copy(
                37,
                pdma_model_contract::dma::CopyCommand {
                    copy_length: length,
                    source: source_endpoint,
                    destination: destination_endpoint,
                    sg_context: contexts[0].raw(),
                },
            ),
            destination,
            same_address: source == destination,
            memory: ports.memory.clone(),
            fence: pdma_model_software::dma::CompletionWord(pdma_model_contract::dma::WordWrite {
                target: pdma_model_contract::dma::WordTarget {
                    address: address.raw(),
                    context: contexts[1].raw(),
                    width: pdma_model_contract::dma::WordTargetWidth::Bits64,
                },
                value: 37,
            }),
        })
    }

    pub(super) async fn submit(&mut self, failure: bool) -> TestResult {
        let mut command = self.command;
        if failure {
            let mut source = Endpoint::linear(u64::MAX - 4096, 0);
            let pdma_model_contract::dma::CommandPayloadValues::Copy(ref mut copy) =
                command.payload
            else {
                return Err("expected Copy".into());
            };
            source.context = copy.source.context;
            copy.source = source;
        }
        self.driver.submit(command).await?;
        self.driver.submit(self.fence.command(command.id)).await?;
        Ok(())
    }

    pub(super) async fn completed(&mut self) -> TestResult<Result<u64, Event>> {
        loop {
            if let Some(response) = self.driver.event().await? {
                return Ok(Err(response));
            }
            if self.driver.fatal().await? != pdma_model_contract::dma::ChannelFatalCode::None {
                return Err("DMA fatal".into());
            }
            if self.fence.is_complete(&self.memory)? {
                return Ok(Ok(self.fence.0.value));
            }
            tokio::task::yield_now().await;
        }
    }

    pub(super) fn bytes(&self, memory: &SharedMemoryAccess) -> TestResult<Vec<u8>> {
        let mut bytes = vec![0; PAYLOAD.len()];
        memory.read(self.destination, MemoryAlignment::ONE, &mut bytes)?;
        Ok(bytes)
    }
}
