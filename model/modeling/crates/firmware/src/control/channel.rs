use crate::Error;
use hw_model_io::memory::{MemoryAccess, MemoryAddress, MemoryAlignment};
use hw_model_io::register::RegisterAccess;
use hw_model_schema::Decode;
use pdma_model_contract::control::{
    ControlQueue, ControlRequest, ControlResponse, ControlRingState, ControlRingStateRaw,
};
use pdma_model_contract::hw_components::control::ControlConfiguration;
use pdma_model_contract::hw_components::control::ControlNotificationRegisters;
use pdma_model_contract::hw_components::control::ControlRegisters;
use pdma_model_contract::ring::{Ring, RingError, RingPointers};
use pdma_model_software::control::ControlQueueLayout;
use pdma_model_software::ring::{MemoryRingIo, RingConfig, SoftRing};

pub(super) struct Channel<'a, B, M> {
    bus: &'a B,
    base: u32,
    descriptor: MemoryAddress,
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
    ) -> Result<Option<Self>, Error<B::Error, M::Error>> {
        if read_word(
            bus,
            base,
            ControlRegisters::CONFIG.offset() + ControlConfiguration::ENABLE.offset(),
        )
        .await?
            == 0
        {
            return Ok(None);
        }
        let mut bytes = [0; 8];
        bus.read(
            address(
                base,
                ControlRegisters::CONFIG.offset() + ControlConfiguration::BOOTSTRAP.offset(),
            )
            .map_err(Error::Ring)?,
            &mut bytes,
        )
        .await
        .map_err(Error::Bus)?;
        let queue_base = u64::from_le_bytes(bytes);
        let layout =
            ControlQueueLayout::new(MemoryAddress::new(queue_base)).map_err(Error::Ring)?;
        let descriptor = MemoryAddress::new(queue_base);
        let command_state = read_state(memory, descriptor, ControlQueue::COMMAND_OFFSET)?;
        let response_state = read_state(memory, descriptor, ControlQueue::RESPONSE_OFFSET)?;
        let command =
            RingConfig::new(command_state.base, command_state.capacity).map_err(Error::Ring)?;
        let response =
            RingConfig::new(response_state.base, response_state.capacity).map_err(Error::Ring)?;
        command
            .validate_pointers(RingPointers {
                producer: command_state.producer,
                consumer: command_state.consumer,
            })
            .map_err(Error::Ring)?;
        response
            .validate_pointers(RingPointers {
                producer: response_state.producer,
                consumer: response_state.consumer,
            })
            .map_err(Error::Ring)?;
        if command_state.consumer != 0
            || response_state.producer != 0
            || response_state.consumer != 0
        {
            return Err(Error::Ring(RingError::Pointers));
        }
        Ok(Some(Self {
            bus,
            base,
            descriptor,
            command: SoftRing::new(command),
            response: SoftRing::new(response),
            command_io: layout.command_io(memory),
            response_io: layout.response_io(memory),
        }))
    }

    pub(super) const fn descriptor(&self) -> MemoryAddress {
        self.descriptor
    }

    pub(super) async fn has_response_space(&self) -> Result<bool, Error<B::Error, M::Error>> {
        self.response
            .has_space(&self.response_io)
            .await
            .map_err(Error::SoftRing)
    }

    pub(super) async fn try_receive(
        &mut self,
    ) -> Result<Option<ControlRequest>, Error<B::Error, M::Error>> {
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
    ) -> Result<(), Error<B::Error, M::Error>> {
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

    async fn notify(&self) -> Result<(), Error<B::Error, M::Error>> {
        self.bus
            .write(
                address(
                    self.base,
                    ControlRegisters::NOTIFICATION.offset()
                        + ControlNotificationRegisters::RESPONSE.offset(),
                )
                .map_err(Error::Ring)?,
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

async fn read_word<B: RegisterAccess + Sync, E>(
    bus: &B,
    base: u32,
    offset: usize,
) -> Result<u32, Error<B::Error, E>> {
    let mut bytes = [0; 4];
    bus.read(address(base, offset).map_err(Error::Ring)?, &mut bytes)
        .await
        .map_err(Error::Bus)?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_state<M: MemoryAccess, E>(
    memory: &M,
    base: MemoryAddress,
    offset: usize,
) -> Result<ControlRingState, Error<E, M::Error>> {
    let address = base
        .raw()
        .checked_add(u64::try_from(offset).map_err(|_| Error::Ring(RingError::Geometry))?)
        .ok_or(Error::Ring(RingError::Geometry))?;
    memory
        .read_buffer::<ControlRingStateRaw>(MemoryAddress::new(address), MemoryAlignment::ONE)
        .map_err(Error::Memory)?
        .decode()
        .map_err(Error::Codec)
}
