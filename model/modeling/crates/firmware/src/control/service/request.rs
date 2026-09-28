use crate::{
    Error,
    control::resources::{ResourceError, Resources},
};
use pdma_model_contract::control::{
    ControlRequest, ControlRequestOpcode, ControlResponse, ControlResponseStatus,
};
use pdma_model_contract::dma::ChannelId;
use pdma_model_contract::ring::RingError;

impl<B: hardware_interface_runtime::RegisterAccess + Sync, M: Sync> super::Service<'_, B, M> {
    pub(super) async fn execute(
        &self,
        resources: &mut Resources,
        (monitor, request): (&super::Monitor<'_, B>, ControlRequest),
    ) -> Result<ControlResponse, Error<B::Error>> {
        match request.opcode {
            ControlRequestOpcode::ResetDma => {
                monitor.reset().await?;
                return Ok(super::response(request));
            }
            ControlRequestOpcode::ResetEvents => {
                let mut response = super::response(request);
                response.status = ControlResponseStatus::InvalidState;
                return Ok(response);
            }
            ControlRequestOpcode::AcquireChannel
            | ControlRequestOpcode::ReleaseChannel
            | ControlRequestOpcode::ResetChannel
            | ControlRequestOpcode::CreateContext
            | ControlRequestOpcode::DrainContext
            | ControlRequestOpcode::AbortContext
            | ControlRequestOpcode::DestroyContext => monitor.healthy().await?,
        }
        let id = ChannelId::new(
            usize::try_from(request.channel).map_err(|_| Error::Ring(RingError::Geometry))?,
        );
        let mut response = ControlResponse {
            id: request.id,
            status: ControlResponseStatus::Success,
            channel: request.channel,
            context: request.context,
        };
        let function = match resources.function(request.function) {
            Ok(function) => function,
            Err(error) => {
                response.status = resource_status(Err(error));
                return Ok(response);
            }
        };
        match request.opcode {
            ControlRequestOpcode::ResetDma | ControlRequestOpcode::ResetEvents => {
                response.status = ControlResponseStatus::InvalidState;
            }
            ControlRequestOpcode::AcquireChannel => {
                match resources.acquire((self.bus, monitor), function).await? {
                    Some(id) => {
                        response.channel = u32::try_from(id.index())
                            .map_err(|_| Error::Ring(RingError::Geometry))?;
                    }
                    None => response.status = ControlResponseStatus::NoResources,
                }
            }
            ControlRequestOpcode::ReleaseChannel => {
                response.status = resource_status(
                    resources
                        .release((self.bus, monitor), (function, id))
                        .await?,
                );
            }
            ControlRequestOpcode::ResetChannel => {
                response.status =
                    resource_status(resources.reset((self.bus, monitor), (function, id)).await?);
            }
            ControlRequestOpcode::CreateContext
            | ControlRequestOpcode::DrainContext
            | ControlRequestOpcode::AbortContext
            | ControlRequestOpcode::DestroyContext => match &resources.contexts {
                Some(contexts) => {
                    contexts
                        .execute(
                            (self.bus, monitor),
                            (&request, &mut response),
                            resources.channels(function),
                        )
                        .await?;
                }
                None => response.status = ControlResponseStatus::InvalidContext,
            },
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
        Err(ResourceError::InvalidFunction) => ControlResponseStatus::InvalidFunction,
    }
}
