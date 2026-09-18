use crate::Error;
use hardware_interface_runtime::RegisterAccess;
use pdma_model_contract::{
    ChannelId, DmaDiscovery, DmaRegisters, FunctionId, RingError, WindowControl,
};
use pdma_model_software::{control::ManagementAddresses, dma::ChannelControl};

#[derive(Clone, Copy)]
pub(super) enum ResourceError {
    InvalidChannel,
    NotAllocated,
    PermissionDenied,
}

struct Channel {
    control: ChannelControl,
    owner: Option<FunctionId>,
}

pub(super) struct Resources {
    channels: Vec<Channel>,
    addresses: ManagementAddresses,
}

impl Resources {
    pub(super) async fn initialize<B: RegisterAccess + Sync>(
        bus: &B,
        addresses: ManagementAddresses,
    ) -> Result<Self, Error<B::Error>> {
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
            channels,
            addresses,
        })
    }

    pub(super) async fn acquire<B: RegisterAccess + Sync>(
        &mut self,
        bus: &B,
        function: FunctionId,
    ) -> Result<Option<ChannelId>, Error<B::Error>> {
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
        Self::set_window(bus, self.addresses, id, true).await?;
        channel.owner = Some(function);
        Ok(Some(id))
    }

    pub(super) async fn reset<B: RegisterAccess + Sync>(
        &mut self,
        bus: &B,
        (function, id): (FunctionId, ChannelId),
    ) -> Result<Result<(), ResourceError>, Error<B::Error>> {
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
        Self::set_window(bus, self.addresses, id, true).await?;
        Ok(Ok(()))
    }

    pub(super) async fn release<B: RegisterAccess + Sync>(
        &mut self,
        bus: &B,
        (function, id): (FunctionId, ChannelId),
    ) -> Result<Result<(), ResourceError>, Error<B::Error>> {
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
        channel.owner = None;
        Ok(Ok(()))
    }

    async fn set_window<B: RegisterAccess + Sync>(
        bus: &B,
        addresses: ManagementAddresses,
        id: ChannelId,
        open: bool,
    ) -> Result<(), Error<B::Error>> {
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
