use super::channel;
use crate::Error;
use hardware_interface_runtime::{MemoryAccess, RegisterAccess};
use pdma_model_contract::{
    ChannelId, ControlRequestOpcode, ControlResponse, ControlResponseStatus, DmaDiscovery,
    DmaRegisters, RingError,
};
use pdma_model_software::{
    control::{InterruptEvent, ManagementAddresses},
    dma::ChannelControl,
};
use std::convert::Infallible;

#[derive(Clone, Copy)]
enum ResourceError {
    InvalidChannel,
    NotAllocated,
}

struct Channel {
    control: ChannelControl,
    allocated: bool,
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
                allocated: false,
            });
        }
        Ok(Self {
            channels,
            addresses,
        })
    }

    async fn acquire<B: RegisterAccess + Sync>(
        &mut self,
        bus: &B,
    ) -> Result<Option<ChannelId>, Error<B::Error>> {
        let Some((index, channel)) = self
            .channels
            .iter_mut()
            .enumerate()
            .find(|(_, channel)| !channel.allocated)
        else {
            return Ok(None);
        };
        let id = ChannelId::new(index);
        Self::set_window(bus, self.addresses, id, true).await?;
        channel.allocated = true;
        Ok(Some(id))
    }

    async fn reset<B: RegisterAccess + Sync>(
        &mut self,
        bus: &B,
        id: ChannelId,
    ) -> Result<Result<(), ResourceError>, Error<B::Error>> {
        let Some(channel) = self.channels.get_mut(id.index()) else {
            return Ok(Err(ResourceError::InvalidChannel));
        };
        if !channel.allocated {
            return Ok(Err(ResourceError::NotAllocated));
        }
        Self::set_window(bus, self.addresses, id, false).await?;
        channel.control.reset(bus).await.map_err(Error::Dma)?;
        Self::set_window(bus, self.addresses, id, true).await?;
        Ok(Ok(()))
    }

    async fn release<B: RegisterAccess + Sync>(
        &mut self,
        bus: &B,
        id: ChannelId,
    ) -> Result<Result<(), ResourceError>, Error<B::Error>> {
        let Some(channel) = self.channels.get_mut(id.index()) else {
            return Ok(Err(ResourceError::InvalidChannel));
        };
        if !channel.allocated {
            return Ok(Err(ResourceError::NotAllocated));
        }
        Self::set_window(bus, self.addresses, id, false).await?;
        channel.control.reset(bus).await.map_err(Error::Dma)?;
        channel.allocated = false;
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

impl Resources {
    pub(super) async fn serve<B: RegisterAccess + Sync, M: MemoryAccess + Sync>(
        mut self,
        bus: &B,
        memory: &M,
        interrupt: &InterruptEvent,
        control_base: u32,
    ) -> Result<Infallible, Error<B::Error>> {
        let mut channel = channel::Channel::attach(bus, memory, interrupt, control_base).await?;
        loop {
            if !channel.has_response_space().await? {
                interrupt.wait().await;
                continue;
            }
            let Some(request) = channel.try_receive().await? else {
                interrupt.wait().await;
                continue;
            };
            let id = ChannelId::new(
                usize::try_from(request.channel).map_err(|_| Error::Ring(RingError::Geometry))?,
            );
            let mut response = ControlResponse {
                id: request.id,
                status: ControlResponseStatus::Success,
                channel: request.channel,
            };
            match request.opcode {
                ControlRequestOpcode::AcquireChannel => match self.acquire(bus).await? {
                    Some(id) => {
                        response.channel = u32::try_from(id.index())
                            .map_err(|_| Error::Ring(RingError::Geometry))?;
                    }
                    None => response.status = ControlResponseStatus::NoResources,
                },
                ControlRequestOpcode::ReleaseChannel => {
                    response.status = resource_status(self.release(bus, id).await?);
                }
                ControlRequestOpcode::ResetChannel => {
                    response.status = resource_status(self.reset(bus, id).await?);
                }
            }
            channel.respond(response).await?;
            tokio::task::yield_now().await;
        }
    }
}

const fn resource_status(result: Result<(), ResourceError>) -> ControlResponseStatus {
    match result {
        Ok(()) => ControlResponseStatus::Success,
        Err(ResourceError::InvalidChannel) => ControlResponseStatus::InvalidChannel,
        Err(ResourceError::NotAllocated) => ControlResponseStatus::NotAllocated,
    }
}
