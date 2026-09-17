use crate::Error;
use hardware_interface_runtime::RegisterAccess;
use pdma_model_contract::{
    ChannelBlock, ChannelRegisters, ControlRequest, ControlRequestOpcode, ControlResponse,
    ControlResponseStatus, DeviceRegisters, WINDOW_CONTROL_BASE,
};

pub(super) async fn execute<B: RegisterAccess + Sync>(
    bus: &B,
    allocated: &mut bool,
    request: ControlRequest,
) -> Result<ControlResponse, Error<B::Error>> {
    let channel = match request.opcode {
        ControlRequestOpcode::AcquireChannel => 0,
        ControlRequestOpcode::ReleaseChannel => request.channel,
    };
    let status = if channel != 0 {
        ControlResponseStatus::InvalidChannel
    } else {
        match request.opcode {
            ControlRequestOpcode::AcquireChannel => {
                if *allocated {
                    ControlResponseStatus::NoResources
                } else {
                    bus.write(WINDOW_CONTROL_BASE, &1_u32.to_le_bytes())
                        .await
                        .map_err(Error::Bus)?;
                    *allocated = true;
                    ControlResponseStatus::Success
                }
            }
            ControlRequestOpcode::ReleaseChannel => {
                if *allocated {
                    bus.write(WINDOW_CONTROL_BASE, &0_u32.to_le_bytes())
                        .await
                        .map_err(Error::Bus)?;
                    let offset = DeviceRegisters::CHANNELS_OFFSET
                        + ChannelBlock::CONTROL.offset()
                        + ChannelRegisters::RESET.offset();
                    let address = u32::try_from(offset)
                        .map_err(|_| Error::Ring(pdma_model_contract::RingError::Geometry))?;
                    bus.write(address, &1_u32.to_le_bytes())
                        .await
                        .map_err(Error::Bus)?;
                    loop {
                        let mut busy = [0; 4];
                        bus.read(address, &mut busy).await.map_err(Error::Bus)?;
                        if u32::from_le_bytes(busy) == 0 {
                            break;
                        }
                        tokio::task::yield_now().await;
                    }
                    *allocated = false;
                    ControlResponseStatus::Success
                } else {
                    ControlResponseStatus::NotAllocated
                }
            }
        }
    };
    Ok(ControlResponse {
        id: request.id,
        status,
        channel,
    })
}
