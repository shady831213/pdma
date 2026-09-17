use crate::Error;
use hardware_interface_runtime::RegisterAccess;
use pdma_model_contract::{
    ChannelId, ControlRequest, ControlRequestOpcode, ControlResponse, ControlResponseStatus,
    DeviceControl, DeviceRegisters, RingError,
};
use pdma_model_software::dma::ChannelControl;

enum State {
    Free,
    Allocated,
    Releasing(ControlRequest),
}

struct Channel {
    control: ChannelControl,
    state: State,
}

pub(super) struct Resources(Vec<Channel>);

impl Resources {
    pub(super) async fn initialize<B: RegisterAccess + Sync>(
        bus: &B,
    ) -> Result<Self, Error<B::Error>> {
        let mut bytes = [0; 4];
        bus.read(
            u32::try_from(DeviceRegisters::CONTROL.offset() + DeviceControl::CHANNEL_NUM.offset())
                .map_err(|_| Error::Ring(RingError::Geometry))?,
            &mut bytes,
        )
        .await
        .map_err(Error::Bus)?;
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
            bus.write(
                id.window_control_address()
                    .ok_or(Error::Ring(RingError::Geometry))?,
                &0_u32.to_le_bytes(),
            )
            .await
            .map_err(Error::Bus)?;
            channels.push(Channel {
                control: ChannelControl::discover(bus, id)
                    .await
                    .map_err(Error::Dma)?,
                state: State::Free,
            });
        }
        Ok(Self(channels))
    }

    pub(super) fn is_releasing(&self) -> bool {
        self.0
            .iter()
            .any(|channel| matches!(channel.state, State::Releasing(_)))
    }

    pub(super) async fn completed<B: RegisterAccess + Sync>(
        &mut self,
        bus: &B,
    ) -> Result<Option<ControlResponse>, Error<B::Error>> {
        for channel in &mut self.0 {
            match channel.state {
                State::Free | State::Allocated => {}
                State::Releasing(request) => {
                    if !channel
                        .control
                        .reset_in_progress(bus)
                        .await
                        .map_err(Error::Dma)?
                    {
                        channel.state = State::Free;
                        return Ok(Some(ControlResponse {
                            id: request.id,
                            status: ControlResponseStatus::Success,
                            channel: request.channel,
                        }));
                    }
                }
            }
        }
        Ok(None)
    }

    pub(super) async fn execute<B: RegisterAccess + Sync>(
        &mut self,
        bus: &B,
        request: ControlRequest,
    ) -> Result<Option<ControlResponse>, Error<B::Error>> {
        let mut response = ControlResponse {
            id: request.id,
            status: ControlResponseStatus::Success,
            channel: request.channel,
        };
        match request.opcode {
            ControlRequestOpcode::AcquireChannel => {
                let Some((index, channel)) = self
                    .0
                    .iter_mut()
                    .enumerate()
                    .find(|(_, channel)| matches!(channel.state, State::Free))
                else {
                    response.status = ControlResponseStatus::NoResources;
                    return Ok(Some(response));
                };
                let id = ChannelId::new(index);
                bus.write(
                    id.window_control_address()
                        .ok_or(Error::Ring(RingError::Geometry))?,
                    &1_u32.to_le_bytes(),
                )
                .await
                .map_err(Error::Bus)?;
                channel.state = State::Allocated;
                response.channel =
                    u32::try_from(index).map_err(|_| Error::Ring(RingError::Geometry))?;
            }
            ControlRequestOpcode::ReleaseChannel => {
                let index = usize::try_from(request.channel)
                    .map_err(|_| Error::Ring(RingError::Geometry))?;
                let Some(channel) = self.0.get_mut(index) else {
                    response.status = ControlResponseStatus::InvalidChannel;
                    return Ok(Some(response));
                };
                match channel.state {
                    State::Free => response.status = ControlResponseStatus::NotAllocated,
                    State::Releasing(_) => response.status = ControlResponseStatus::Busy,
                    State::Allocated => {
                        bus.write(
                            ChannelId::new(index)
                                .window_control_address()
                                .ok_or(Error::Ring(RingError::Geometry))?,
                            &0_u32.to_le_bytes(),
                        )
                        .await
                        .map_err(Error::Bus)?;
                        channel
                            .control
                            .request_reset(bus)
                            .await
                            .map_err(Error::Dma)?;
                        channel.state = State::Releasing(request);
                        return Ok(None);
                    }
                }
            }
        }
        Ok(Some(response))
    }
}
