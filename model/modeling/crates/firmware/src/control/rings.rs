use crate::Error;
use hardware_interface_runtime::{MemoryAccess, MemoryAddress, MemoryAlignment, RegisterAccess};
use hardware_interface_schema::Decode;
use pdma_model_contract::{
    CONTROL_CHANNEL_BASE, ControlQueue, ControlQueueRaw, ControlRegisters, ControlRequest,
    ControlResponse, ControlRingState, Ring, RingError, RingPointers,
};
use pdma_model_software::ring::{MemoryRingIo, RingConfig, SoftRing};

pub(super) struct Queues {
    pub(super) command: SoftRing<ControlRequest>,
    pub(super) response: SoftRing<ControlResponse>,
    base: u64,
}

impl Queues {
    pub(super) async fn attach<B: RegisterAccess + Sync, M: MemoryAccess + Sync>(
        bus: &B,
        memory: &M,
    ) -> Result<Self, Error<B::Error>> {
        let mut bytes = [0; 8];
        bus.read(
            address(ControlRegisters::BOOTSTRAP.offset()).map_err(Error::Ring)?,
            &mut bytes,
        )
        .await
        .map_err(Error::Bus)?;
        let base = u64::from_le_bytes(bytes);
        let alignment = u64::try_from(ControlQueue::RECORD_ALIGNMENT)
            .map_err(|_| Error::Ring(RingError::Geometry))?;
        if base % alignment != 0 {
            return Err(Error::Ring(RingError::Geometry));
        }
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
            .advance_consumer(
                RingPointers {
                    producer: queue.command.producer,
                    consumer: queue.command.consumer,
                },
                0,
            )
            .map_err(Error::Ring)?;
        response
            .advance_consumer(
                RingPointers {
                    producer: queue.response.producer,
                    consumer: queue.response.consumer,
                },
                0,
            )
            .map_err(Error::Ring)?;
        if queue.command.consumer != 0
            || queue.response.producer != 0
            || queue.response.consumer != 0
        {
            return Err(Error::Ring(RingError::Pointers));
        }
        Ok(Self {
            command: SoftRing::new(command).map_err(Error::Cpu)?,
            response: SoftRing::new(response).map_err(Error::Cpu)?,
            base,
        })
    }

    pub(super) fn io<'a, M>(
        &self,
        memory: &'a M,
        offset: usize,
    ) -> Result<MemoryRingIo<'a, M>, RingError> {
        let ring = self
            .base
            .checked_add(u64::try_from(offset).map_err(|_| RingError::Geometry)?)
            .ok_or(RingError::Geometry)?;
        let producer = ring
            .checked_add(
                u64::try_from(ControlRingState::PRODUCER_OFFSET)
                    .map_err(|_| RingError::Geometry)?,
            )
            .ok_or(RingError::Geometry)?;
        let consumer = ring
            .checked_add(
                u64::try_from(ControlRingState::CONSUMER_OFFSET)
                    .map_err(|_| RingError::Geometry)?,
            )
            .ok_or(RingError::Geometry)?;
        Ok(MemoryRingIo::new(
            memory,
            MemoryAddress::new(producer),
            MemoryAddress::new(consumer),
        ))
    }
}

fn address(offset: usize) -> Result<u32, RingError> {
    CONTROL_CHANNEL_BASE
        .checked_add(u32::try_from(offset).map_err(|_| RingError::Geometry)?)
        .ok_or(RingError::Geometry)
}

pub(super) async fn read_word<B: RegisterAccess + Sync>(
    bus: &B,
    offset: usize,
) -> Result<u32, Error<B::Error>> {
    let mut bytes = [0; 4];
    bus.read(address(offset).map_err(Error::Ring)?, &mut bytes)
        .await
        .map_err(Error::Bus)?;
    Ok(u32::from_le_bytes(bytes))
}

pub(super) async fn notify<B: RegisterAccess + Sync>(bus: &B) -> Result<(), Error<B::Error>> {
    bus.write(
        address(ControlRegisters::RESPONSE.offset()).map_err(Error::Ring)?,
        &1_u32.to_le_bytes(),
    )
    .await
    .map_err(Error::Bus)
}
