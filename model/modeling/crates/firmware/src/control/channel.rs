use crate::Error;
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment, RegisterAccess};
use hardware_interface_schema::Decode;
use pdma_model_contract::{
    CONTROL_CHANNEL_BASE, ControlQueueRaw, ControlRegisters, ControlRequest, ControlResponse, Ring,
    RingError, RingPointers,
};
use pdma_model_software::control::{ControlQueueLayout, InterruptEvent};
use pdma_model_software::ring::{MemoryRingIo, RingConfig, SoftRing};

pub(super) struct Channel<'a, B, M> {
    bus: &'a B,
    command: SoftRing<ControlRequest>,
    response: SoftRing<ControlResponse>,
    command_io: MemoryRingIo<'a, M>,
    response_io: MemoryRingIo<'a, M>,
}

impl<'a, B: RegisterAccess + Sync, M: MemoryAccess + Sync> Channel<'a, B, M> {
    pub(super) async fn attach(
        bus: &'a B,
        memory: &'a M,
        interrupt: &'a InterruptEvent,
    ) -> Result<Self, Error<B::Error>> {
        while read_word(bus, ControlRegisters::ENABLE.offset()).await? == 0 {
            interrupt.wait().await;
        }
        let mut bytes = [0; 8];
        bus.read(
            address(ControlRegisters::BOOTSTRAP.offset()).map_err(Error::Ring)?,
            &mut bytes,
        )
        .await
        .map_err(Error::Bus)?;
        let base = u64::from_le_bytes(bytes);
        let layout = ControlQueueLayout::new(MemoryAddress::new(base)).map_err(Error::Ring)?;
        let queue = memory
            .read_typed::<ControlQueueRaw>(MemoryAddress::new(base), MemoryAlignment::ONE)
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
        Ok(Self {
            bus,
            command: SoftRing::new(command).map_err(Error::Cpu)?,
            response: SoftRing::new(response).map_err(Error::Cpu)?,
            command_io: layout.command_io(memory),
            response_io: layout.response_io(memory),
        })
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
                address(ControlRegisters::RESPONSE.offset()).map_err(Error::Ring)?,
                &1_u32.to_le_bytes(),
            )
            .await
            .map_err(Error::Bus)
    }
}

fn address(offset: usize) -> Result<u32, RingError> {
    CONTROL_CHANNEL_BASE
        .checked_add(u32::try_from(offset).map_err(|_| RingError::Geometry)?)
        .ok_or(RingError::Geometry)
}

async fn read_word<B: RegisterAccess + Sync>(
    bus: &B,
    offset: usize,
) -> Result<u32, Error<B::Error>> {
    let mut bytes = [0; 4];
    bus.read(address(offset).map_err(Error::Ring)?, &mut bytes)
        .await
        .map_err(Error::Bus)?;
    Ok(u32::from_le_bytes(bytes))
}
