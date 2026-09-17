use crate::Error;
use hardware_interface_runtime::RegisterAccess;
use hardware_interface_schema::RecordLayout;
use pdma_model_contract::{
    CONTROL_CHANNEL_BASE, ControlRingRegisters, Ring, RingError, RingPointers,
};
use std::marker::PhantomData;

pub(super) struct Snapshot<E> {
    base: u64,
    capacity: u32,
    pub(super) pointers: RingPointers,
    entry: PhantomData<E>,
}

impl<E: RecordLayout> Ring for Snapshot<E> {
    type Entry = E;
    fn base(&self) -> u64 {
        self.base
    }
    fn capacity(&self) -> u32 {
        self.capacity
    }
}

impl<E: RecordLayout> Snapshot<E> {
    pub(super) async fn read<B: RegisterAccess + Sync>(
        bus: &B,
        offset: usize,
    ) -> Result<Self, Error<B::Error>> {
        let mut base = [0; 8];
        bus.read(
            address(offset + ControlRingRegisters::BASE.offset()).map_err(Error::Ring)?,
            &mut base,
        )
        .await
        .map_err(Error::Bus)?;
        let snapshot = Self {
            base: u64::from_le_bytes(base),
            capacity: read_word(bus, offset + ControlRingRegisters::CAPACITY.offset()).await?,
            pointers: RingPointers {
                producer: read_word(bus, offset + ControlRingRegisters::PRODUCER.offset()).await?,
                consumer: read_word(bus, offset + ControlRingRegisters::CONSUMER.offset()).await?,
            },
            entry: PhantomData,
        };
        snapshot.validate_geometry().map_err(Error::Ring)?;
        Ok(snapshot)
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

pub(super) async fn write_word<B: RegisterAccess + Sync>(
    bus: &B,
    offset: usize,
    value: u32,
) -> Result<(), Error<B::Error>> {
    bus.write(address(offset).map_err(Error::Ring)?, &value.to_le_bytes())
        .await
        .map_err(Error::Bus)
}
