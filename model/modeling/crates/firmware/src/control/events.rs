use crate::Error;
use hardware_interface_io::memory::{MemoryAccess, MemoryAddress, MemoryAlignment};
use hardware_interface_io::register::RegisterAccess;
use hardware_interface_schema::Decode;
use pdma_model_contract::hw_components::control::ControlNotificationRegisters;
use pdma_model_contract::{
    control::{ControlEvent, ControlQueue, ControlResponseStatus, ControlRingStateRaw},
    hw_components::control::{ControlRegisters, EventRingErrorCode, EventRingErrorValues},
    ring::RingError,
};
use pdma_model_software::{
    control::ControlQueueLayout,
    ring::{MemoryRingIo, RingConfig, SoftRing, SoftRingError},
};

pub(super) struct Events<'a, B, M> {
    bus: &'a B,
    memory: &'a M,
    base: u32,
    descriptor: MemoryAddress,
    io: MemoryRingIo<'a, M>,
    ring: Option<SoftRing<ControlEvent>>,
    error: Option<EventRingErrorCode>,
}

impl<'a, B: RegisterAccess + Sync, M: MemoryAccess + Sync> Events<'a, B, M> {
    pub(super) async fn attach(
        access: (&'a B, &'a M),
        location: (u32, MemoryAddress),
    ) -> Result<Self, Error<B::Error, M::Error>> {
        let (bus, memory) = access;
        let (base, descriptor) = location;
        let layout = ControlQueueLayout::new(descriptor).map_err(Error::Ring)?;
        let mut events = Self {
            bus,
            memory,
            base,
            descriptor,
            io: layout.event_io(memory),
            ring: None,
            error: None,
        };
        match events.load() {
            Ok(ring) => events.ring = Some(ring),
            Err(error) => events.fail(error).await?,
        }
        Ok(events)
    }

    pub(super) async fn publish(
        &mut self,
        event: ControlEvent,
    ) -> Result<(), Error<B::Error, M::Error>> {
        let Some(ring) = &mut self.ring else {
            return Ok(());
        };
        match ring.try_push(&self.io, event).await {
            Ok(true) => {
                self.write(
                    ControlRegisters::NOTIFICATION.offset()
                        + ControlNotificationRegisters::EVENT.offset(),
                    1,
                )
                .await
            }
            Ok(false) => self.fail(EventRingErrorCode::Overflow).await,
            Err(error) => self.fail(classify(&error)).await,
        }
    }

    pub(super) async fn reset(
        &mut self,
    ) -> Result<ControlResponseStatus, Error<B::Error, M::Error>> {
        if self.error.is_none() {
            return Ok(ControlResponseStatus::InvalidState);
        }
        let Ok(ring) = self.load() else {
            return Ok(ControlResponseStatus::InvalidState);
        };
        self.write(
            ControlRegisters::NOTIFICATION.offset()
                + ControlNotificationRegisters::EVENT_ERROR.offset(),
            0,
        )
        .await?;
        self.ring = Some(ring);
        self.error = None;
        Ok(ControlResponseStatus::Success)
    }

    fn load(&self) -> Result<SoftRing<ControlEvent>, EventRingErrorCode> {
        let offset = u64::try_from(ControlQueue::EVENT_OFFSET)
            .map_err(|_| EventRingErrorCode::InvalidRing)?;
        let address = self
            .descriptor
            .raw()
            .checked_add(offset)
            .ok_or(EventRingErrorCode::InvalidRing)?;
        let state = self
            .memory
            .read_buffer::<ControlRingStateRaw>(MemoryAddress::new(address), MemoryAlignment::ONE)
            .map_err(|_| EventRingErrorCode::AccessError)?
            .decode()
            .map_err(|_| EventRingErrorCode::InvalidRing)?;
        let config = RingConfig::new(state.base, state.capacity)
            .map_err(|_| EventRingErrorCode::InvalidRing)?;
        if state.producer != 0 || state.consumer != 0 {
            return Err(EventRingErrorCode::InvalidRing);
        }
        Ok(SoftRing::new(config))
    }

    async fn fail(&mut self, error: EventRingErrorCode) -> Result<(), Error<B::Error, M::Error>> {
        if self.error.is_some() {
            return Ok(());
        }
        self.ring = None;
        self.error = Some(error);
        self.write(
            ControlRegisters::NOTIFICATION.offset()
                + ControlNotificationRegisters::EVENT_ERROR.offset(),
            EventRingErrorValues::RESET.with_code(error).raw(),
        )
        .await?;
        self.write(
            ControlRegisters::NOTIFICATION.offset() + ControlNotificationRegisters::EVENT.offset(),
            1,
        )
        .await
    }

    async fn write(&self, offset: usize, value: u32) -> Result<(), Error<B::Error, M::Error>> {
        let address = self
            .base
            .checked_add(u32::try_from(offset).map_err(|_| Error::Ring(RingError::Geometry))?)
            .ok_or(Error::Ring(RingError::Geometry))?;
        self.bus
            .write(address, &value.to_le_bytes())
            .await
            .map_err(Error::Bus)
    }
}

const fn classify<E>(error: &SoftRingError<E, E>) -> EventRingErrorCode {
    match error {
        SoftRingError::Cursor(_) | SoftRingError::Memory(_) => EventRingErrorCode::AccessError,
        SoftRingError::Codec(_)
        | SoftRingError::Ring(_)
        | SoftRingError::Busy
        | SoftRingError::Idle
        | SoftRingError::CursorChanged => EventRingErrorCode::InvalidRing,
    }
}
