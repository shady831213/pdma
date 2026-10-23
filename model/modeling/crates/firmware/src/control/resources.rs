use super::monitor::Monitor;
use crate::{Configuration, Error};
use hardware_interface_runtime::RegisterAccess;
use pdma_model_contract::dma::{ChannelId, DmaDiscovery, DmaRegisters};
use pdma_model_contract::hw_components::windows::WindowControl;
use pdma_model_contract::memory::FunctionId;
use pdma_model_contract::ring::RingError;
use pdma_model_software::{control::ManagementAddresses, dma::ChannelControl};

#[derive(Clone, Copy)]
pub(super) enum ResourceError {
    InvalidFunction,
    InvalidChannel,
    NotAllocated,
    PermissionDenied,
}

struct Channel {
    control: ChannelControl,
    owner: Option<FunctionId>,
}

pub(super) struct Resources {
    pub(super) contexts: Option<super::contexts::Contexts>,
    channels: Vec<Channel>,
    addresses: ManagementAddresses,
    function_count: u32,
}

impl Resources {
    pub(super) async fn initialize<B: RegisterAccess + Sync, M>(
        bus: &B,
        config: Configuration,
    ) -> Result<Self, Error<B::Error, M>> {
        let addresses = config.addresses;
        let function_count =
            u32::try_from(config.function_count).map_err(|_| Error::FunctionCount)?;
        if function_count == 0 {
            return Err(Error::FunctionCount);
        }
        let mut bytes = [0; 4];
        let discovery = addresses
            .dma
            .checked_add(
                u32::try_from(
                    DmaRegisters::DISCOVERY.offset() + DmaDiscovery::CHANNEL_NUM.offset(),
                )
                .map_err(|_| Error::Ring(RingError::Geometry))?,
            )
            .ok_or(Error::Ring(RingError::Geometry))?;
        bus.read(discovery, &mut bytes).await.map_err(Error::Bus)?;
        let count = usize::try_from(u32::from_le_bytes(bytes))
            .map_err(|_| Error::Ring(RingError::Geometry))?;
        if count == 0 {
            return Err(Error::Ring(RingError::Geometry));
        }
        let mut channels = Vec::new();
        channels
            .try_reserve_exact(count)
            .map_err(Error::Allocation)?;
        for index in 0..count {
            let id = ChannelId::new(index);
            Self::set_window(bus, addresses, id, false).await?;
            channels.push(Channel {
                control: ChannelControl::discover_at(bus, id, addresses.dma)
                    .await
                    .map_err(Error::Dma)?,
                owner: None,
            });
        }
        Ok(Self {
            contexts: match addresses.contexts {
                Some(base) => Some(super::contexts::Contexts::discover(bus, base, count).await?),
                None => None,
            },
            channels,
            addresses,
            function_count,
        })
    }

    pub(super) const fn channel_count(&self) -> usize {
        self.channels.len()
    }

    pub(super) const fn function(&self, raw: u32) -> Result<FunctionId, ResourceError> {
        if raw < self.function_count {
            Ok(FunctionId::new(raw))
        } else {
            Err(ResourceError::InvalidFunction)
        }
    }

    pub(super) fn channels(&self, function: FunctionId) -> impl Iterator<Item = ChannelId> + '_ {
        self.channels
            .iter()
            .enumerate()
            .filter_map(move |(index, channel)| {
                (channel.owner == Some(function)).then_some(ChannelId::new(index))
            })
    }

    pub(super) async fn acquire<B: RegisterAccess + Sync, M>(
        &mut self,
        (bus, monitor): (&B, &Monitor<'_, B>),
        function: FunctionId,
    ) -> Result<Option<ChannelId>, Error<B::Error, M>> {
        let Some((index, channel)) = self
            .channels
            .iter_mut()
            .enumerate()
            .find(|(_, channel)| channel.owner.is_none())
        else {
            return Ok(None);
        };
        let id = ChannelId::new(index);
        let owner = self
            .addresses
            .window_control(id)
            .and_then(|base| base.checked_add(u32::try_from(WindowControl::OWNER.offset()).ok()?))
            .ok_or(Error::Ring(RingError::Geometry))?;
        bus.write(owner, &function.raw().to_le_bytes())
            .await
            .map_err(Error::Bus)?;
        monitor.healthy().await?;
        Self::set_window(bus, self.addresses, id, true).await?;
        monitor.healthy().await?;
        channel.owner = Some(function);
        Ok(Some(id))
    }

    pub(super) async fn reset<B: RegisterAccess + Sync, M>(
        &mut self,
        (bus, monitor): (&B, &Monitor<'_, B>),
        (function, id): (FunctionId, ChannelId),
    ) -> Result<Result<(), ResourceError>, Error<B::Error, M>> {
        let Some(channel) = self.channels.get_mut(id.index()) else {
            return Ok(Err(ResourceError::InvalidChannel));
        };
        if channel.owner.is_none() {
            return Ok(Err(ResourceError::NotAllocated));
        }
        if channel.owner != Some(function) {
            return Ok(Err(ResourceError::PermissionDenied));
        }
        Self::set_window(bus, self.addresses, id, false).await?;
        channel.control.reset(bus).await.map_err(Error::Dma)?;
        monitor.healthy().await?;
        Self::set_window(bus, self.addresses, id, true).await?;
        monitor.healthy().await?;
        Ok(Ok(()))
    }

    pub(super) async fn release<B: RegisterAccess + Sync, M>(
        &mut self,
        (bus, monitor): (&B, &Monitor<'_, B>),
        (function, id): (FunctionId, ChannelId),
    ) -> Result<Result<(), ResourceError>, Error<B::Error, M>> {
        let Some(channel) = self.channels.get_mut(id.index()) else {
            return Ok(Err(ResourceError::InvalidChannel));
        };
        if channel.owner.is_none() {
            return Ok(Err(ResourceError::NotAllocated));
        }
        if channel.owner != Some(function) {
            return Ok(Err(ResourceError::PermissionDenied));
        }
        Self::set_window(bus, self.addresses, id, false).await?;
        channel.control.reset(bus).await.map_err(Error::Dma)?;
        monitor.healthy().await?;
        channel.owner = None;
        Ok(Ok(()))
    }

    async fn set_window<B: RegisterAccess + Sync, M>(
        bus: &B,
        addresses: ManagementAddresses,
        id: ChannelId,
        open: bool,
    ) -> Result<(), Error<B::Error, M>> {
        bus.write(
            addresses
                .window_control(id)
                .ok_or(Error::Ring(RingError::Geometry))?,
            &u32::from(open).to_le_bytes(),
        )
        .await
        .map_err(Error::Bus)
    }
}
