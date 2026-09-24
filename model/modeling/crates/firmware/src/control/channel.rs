use crate::Error;
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment, RegisterAccess};
use hardware_interface_schema::Decode;
use pdma_model_contract::control::{ControlQueueRaw, ControlRequest, ControlResponse};
use pdma_model_contract::hw_components::control::ControlRegisters;
use pdma_model_contract::ring::{Ring, RingError, RingPointers};
use pdma_model_software::control::ControlQueueLayout;
use pdma_model_software::ring::{MemoryRingIo, RingConfig, SoftRing};

pub(super) struct Channel<'a, B, M> {
    bus: &'a B,
    base: u32,
    command: SoftRing<ControlRequest>,
    response: SoftRing<ControlResponse>,
    command_io: MemoryRingIo<'a, M>,
    response_io: MemoryRingIo<'a, M>,
}

impl<'a, B: RegisterAccess + Sync, M: MemoryAccess + Sync> Channel<'a, B, M> {
    pub(super) async fn try_attach(
        bus: &'a B,
        memory: &'a M,
        base: u32,
    ) -> Result<Option<Self>, Error<B::Error>> {
        if read_word(bus, base, ControlRegisters::ENABLE.offset()).await? == 0 {
            return Ok(None);
        }
        let mut bytes = [0; 8];
        bus.read(
            address(base, ControlRegisters::BOOTSTRAP.offset()).map_err(Error::Ring)?,
            &mut bytes,
        )
        .await
        .map_err(Error::Bus)?;
        let queue_base = u64::from_le_bytes(bytes);
        let layout =
            ControlQueueLayout::new(MemoryAddress::new(queue_base)).map_err(Error::Ring)?;
        let queue = memory
            .read_typed::<ControlQueueRaw>(MemoryAddress::new(queue_base), MemoryAlignment::ONE)
            .map_err(Error::Memory)?
            .decode()
            .map_err(Error::Codec)?;
        let command =
            RingConfig::new(queue.command.base, queue.command.capacity).map_err(Error::Ring)?;
        let response =
            RingConfig::new(queue.response.base, queue.response.capacity).map_err(Error::Ring)?;
        command
            .validate_pointers(RingPointers {
                producer: queue.command.producer,
                consumer: queue.command.consumer,
            })
            .map_err(Error::Ring)?;
        response
            .validate_pointers(RingPointers {
                producer: queue.response.producer,
                consumer: queue.response.consumer,
            })
            .map_err(Error::Ring)?;
        if queue.command.consumer != 0
            || queue.response.producer != 0
            || queue.response.consumer != 0
        {
            return Err(Error::Ring(RingError::Pointers));
        }
        Ok(Some(Self {
            bus,
            base,
            command: SoftRing::new(command),
            response: SoftRing::new(response),
            command_io: layout.command_io(memory),
            response_io: layout.response_io(memory),
        }))
    }

    pub(super) async fn has_response_space(&self) -> Result<bool, Error<B::Error>> {
        self.response
            .has_space(&self.response_io)
            .await
            .map_err(Error::SoftRing)
    }

    pub(super) async fn try_receive(&mut self) -> Result<Option<ControlRequest>, Error<B::Error>> {
        let request = self
            .command
            .peek(&self.command_io)
            .await
            .map_err(Error::SoftRing)?;
        if request.is_some() {
            self.command
                .consume(&self.command_io)
                .await
                .map_err(Error::SoftRing)?;
            self.notify().await?;
        }
        Ok(request)
    }

    pub(super) async fn respond(
        &mut self,
        response: ControlResponse,
    ) -> Result<(), Error<B::Error>> {
        // The service checked space before this publication; no other producer writes this ring.
        if !self
            .response
            .try_push(&self.response_io, response)
            .await
            .map_err(Error::SoftRing)?
        {
            return Err(Error::Ring(RingError::Pointers));
        }
        self.notify().await
    }

    async fn notify(&self) -> Result<(), Error<B::Error>> {
        self.bus
            .write(
                address(self.base, ControlRegisters::RESPONSE.offset()).map_err(Error::Ring)?,
                &1_u32.to_le_bytes(),
            )
            .await
            .map_err(Error::Bus)
    }
}

fn address(base: u32, offset: usize) -> Result<u32, RingError> {
    base.checked_add(u32::try_from(offset).map_err(|_| RingError::Geometry)?)
        .ok_or(RingError::Geometry)
}

async fn read_word<B: RegisterAccess + Sync>(
    bus: &B,
    base: u32,
    offset: usize,
) -> Result<u32, Error<B::Error>> {
    let mut bytes = [0; 4];
    bus.read(address(base, offset).map_err(Error::Ring)?, &mut bytes)
        .await
        .map_err(Error::Bus)?;
    Ok(u32::from_le_bytes(bytes))
}
