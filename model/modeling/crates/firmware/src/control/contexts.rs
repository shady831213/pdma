use crate::Error;
use hardware_interface_runtime::RegisterAccess;
use pdma_model_contract::control::{
    ControlRequest, ControlRequestOpcode, ControlResponse, ControlResponseStatus,
};
use pdma_model_contract::dma::ChannelId;
use pdma_model_contract::hw_components::context::{
    ContextAttributeRegisters, ContextEntryRegisters, ContextManagementRegisters,
    ContextRequestAction, ContextRequestValues, ContextResultState, ContextResultValues,
    ContextTableRegisters,
};
use pdma_model_contract::ring::RingError;

pub(super) struct Contexts {
    base: u32,
    count: usize,
    channel_count: usize,
}

impl Contexts {
    pub(super) async fn discover<B: RegisterAccess + Sync>(
        bus: &B,
        base: u32,
        channel_count: usize,
    ) -> Result<Self, Error<B::Error>> {
        let mut table = Self {
            base,
            count: 0,
            channel_count,
        };
        table.count = usize::try_from(
            table
                .read(bus, ContextManagementRegisters::CAPACITY.offset())
                .await?,
        )
        .map_err(|_| Error::Ring(RingError::Geometry))?;
        Ok(table)
    }

    pub(super) async fn execute<B: RegisterAccess + Sync>(
        &self,
        bus: &B,
        (request, response): (&ControlRequest, &mut ControlResponse),
        channels: impl Iterator<Item = ChannelId> + Send,
    ) -> Result<(), Error<B::Error>> {
        let (action, destroy) = match request.opcode {
            ControlRequestOpcode::CreateContext => {
                return self.create(bus, (request, response)).await;
            }
            ControlRequestOpcode::DrainContext => (ContextRequestAction::Drain, false),
            ControlRequestOpcode::DestroyContext => (ContextRequestAction::Drain, true),
            ControlRequestOpcode::AbortContext => (ContextRequestAction::Abort, false),
            ControlRequestOpcode::AcquireChannel
            | ControlRequestOpcode::ReleaseChannel
            | ControlRequestOpcode::ResetChannel => return Ok(()),
        };
        let index =
            usize::try_from(request.context).map_err(|_| Error::Ring(RingError::Geometry))?;
        if index >= self.count {
            response.status = ControlResponseStatus::InvalidContext;
            return Ok(());
        }
        let entry = self.entry(index)?;
        if self
            .read(bus, entry + ContextEntryRegisters::VALID.offset())
            .await?
            == 0
        {
            response.status = ControlResponseStatus::InvalidContext;
            return Ok(());
        }
        if self
            .read(bus, entry + ContextEntryRegisters::OWNER.offset())
            .await?
            != request.function
        {
            response.status = ControlResponseStatus::PermissionDenied;
            return Ok(());
        }
        let value = ContextRequestValues::RESET
            .with_context_id(request.context)
            .map_err(Error::RegisterCodec)?
            .with_action(action);
        bus.write(
            self.address(ContextManagementRegisters::REQUEST.offset())?,
            &value.raw().to_le_bytes(),
        )
        .await
        .map_err(Error::Bus)?;
        loop {
            let raw = self
                .read(bus, ContextManagementRegisters::RESULT.offset())
                .await?;
            let state = ContextResultValues::new(raw)
                .state()
                .map_err(Error::RegisterCodec)?;
            response.status = match state {
                ContextResultState::Pending => {
                    tokio::task::yield_now().await;
                    continue;
                }
                ContextResultState::Success => {
                    if destroy {
                        self.write(bus, (entry + ContextEntryRegisters::VALID.offset(), 0))
                            .await?;
                    }
                    ControlResponseStatus::Success
                }
                ContextResultState::InvalidContext => ControlResponseStatus::InvalidContext,
                ContextResultState::Busy => ControlResponseStatus::ContextBusy,
                ContextResultState::Idle | ContextResultState::Failed => {
                    let failed = self.failed_channels(bus, channels).await?;
                    response.channel = failed
                        .first()
                        .and_then(|id| u32::try_from(id.index()).ok())
                        .unwrap_or(u32::MAX);
                    ControlResponseStatus::MaintenanceFailed
                }
            };
            return Ok(());
        }
    }

    async fn create<B: RegisterAccess + Sync>(
        &self,
        bus: &B,
        (request, response): (&ControlRequest, &mut ControlResponse),
    ) -> Result<(), Error<B::Error>> {
        if request.attributes.validate().is_err() {
            response.status = ControlResponseStatus::InvalidAttributes;
            return Ok(());
        }
        for index in 0..self.count {
            let entry = self.entry(index)?;
            if self
                .read(bus, entry + ContextEntryRegisters::VALID.offset())
                .await?
                != 0
            {
                continue;
            }
            self.write(
                bus,
                (
                    entry + ContextEntryRegisters::OWNER.offset(),
                    request.function,
                ),
            )
            .await?;
            let attributes = entry + ContextEntryRegisters::ATTRIBUTES.offset();
            self.write(
                bus,
                (
                    attributes + ContextAttributeRegisters::ADDRESS_SPACE.offset(),
                    request.attributes.address_space.raw(),
                ),
            )
            .await?;
            self.write(
                bus,
                (
                    attributes + ContextAttributeRegisters::PASID.offset(),
                    request.attributes.pasid,
                ),
            )
            .await?;
            self.write(bus, (entry + ContextEntryRegisters::VALID.offset(), 1))
                .await?;
            response.context =
                u32::try_from(index).map_err(|_| Error::Ring(RingError::Geometry))?;
            return Ok(());
        }
        response.status = ControlResponseStatus::NoResources;
        Ok(())
    }

    fn entry<E>(&self, index: usize) -> Result<usize, Error<E>> {
        ContextTableRegisters::for_channels(self.count, self.channel_count)
            .map_err(|_| Error::Ring(RingError::Geometry))?
            .entries()
            .offset(index)
            .ok_or(Error::Ring(RingError::Geometry))
    }

    async fn failed_channels<B: RegisterAccess + Sync>(
        &self,
        bus: &B,
        channels: impl Iterator<Item = ChannelId> + Send,
    ) -> Result<Vec<ChannelId>, Error<B::Error>> {
        let layout = ContextTableRegisters::for_channels(self.count, self.channel_count)
            .map_err(|_| Error::Ring(RingError::Geometry))?;
        let mut failed = Vec::new();
        let mut cached = None;
        for channel in channels {
            let index = channel.index() / 32;
            let bits = match cached {
                Some((previous, bits)) if previous == index => bits,
                Some(_) | None => {
                    let bits = self
                        .read(
                            bus,
                            layout
                                .failed()
                                .offset(index)
                                .ok_or(Error::Ring(RingError::Geometry))?,
                        )
                        .await?;
                    cached = Some((index, bits));
                    bits
                }
            };
            if bits & (1 << (channel.index() % 32)) != 0 {
                failed.try_reserve(1).map_err(Error::Allocation)?;
                failed.push(channel);
            }
        }
        Ok(failed)
    }

    fn address<E>(&self, offset: usize) -> Result<u32, Error<E>> {
        self.base
            .checked_add(u32::try_from(offset).map_err(|_| Error::Ring(RingError::Geometry))?)
            .ok_or(Error::Ring(RingError::Geometry))
    }

    async fn read<B: RegisterAccess + Sync>(
        &self,
        bus: &B,
        offset: usize,
    ) -> Result<u32, Error<B::Error>> {
        let mut bytes = [0; 4];
        bus.read(self.address(offset)?, &mut bytes)
            .await
            .map_err(Error::Bus)?;
        Ok(u32::from_le_bytes(bytes))
    }

    async fn write<B: RegisterAccess + Sync>(
        &self,
        bus: &B,
        (offset, value): (usize, u32),
    ) -> Result<(), Error<B::Error>> {
        bus.write(self.address(offset)?, &value.to_le_bytes())
            .await
            .map_err(Error::Bus)
    }
}
