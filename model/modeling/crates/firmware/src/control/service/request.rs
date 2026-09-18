use crate::{
    Error,
    control::resources::{ResourceError, Resources},
};
use pdma_model_contract::{
    ChannelId, ControlRequest, ControlRequestOpcode, ControlResponse, ControlResponseStatus,
    FunctionId, RingError,
};

impl<B: hardware_interface_runtime::RegisterAccess + Sync, M: Sync> super::Service<'_, B, M> {
    pub(super) async fn execute(
        &self,
        resources: &mut Resources,
        (function, request): (FunctionId, ControlRequest),
    ) -> Result<ControlResponse, Error<B::Error>> {
        let id = ChannelId::new(
            usize::try_from(request.channel).map_err(|_| Error::Ring(RingError::Geometry))?,
        );
        let mut response = ControlResponse {
            id: request.id,
            status: ControlResponseStatus::Success,
            channel: request.channel,
        };
        match request.opcode {
            ControlRequestOpcode::AcquireChannel => {
                match resources.acquire(self.bus, function).await? {
                    Some(id) => {
                        response.channel = u32::try_from(id.index())
                            .map_err(|_| Error::Ring(RingError::Geometry))?;
                    }
                    None => response.status = ControlResponseStatus::NoResources,
                }
            }
            ControlRequestOpcode::ReleaseChannel => {
                response.status =
                    resource_status(resources.release(self.bus, (function, id)).await?);
            }
            ControlRequestOpcode::ResetChannel => {
                response.status = resource_status(resources.reset(self.bus, (function, id)).await?);
            }
        }
        Ok(response)
    }
}

const fn resource_status(result: Result<(), ResourceError>) -> ControlResponseStatus {
    match result {
        Ok(()) => ControlResponseStatus::Success,
        Err(ResourceError::InvalidChannel) => ControlResponseStatus::InvalidChannel,
        Err(ResourceError::NotAllocated) => ControlResponseStatus::NotAllocated,
        Err(ResourceError::PermissionDenied) => ControlResponseStatus::PermissionDenied,
    }
}
