use crate::Error;
use hardware_interface_runtime::RegisterAccess;
use pdma_model_contract::{
    ChannelId, ControlRequest, ControlRequestOpcode, ControlResponse, ControlResponseStatus,
    WINDOW_CONTROL_BASE,
};
use pdma_model_software::dma::ChannelControl;

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
                    ChannelControl::discover(bus, ChannelId::new(0))
                        .await
                        .map_err(Error::Dma)?
                        .reset(bus)
                        .await
                        .map_err(Error::Dma)?;
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
