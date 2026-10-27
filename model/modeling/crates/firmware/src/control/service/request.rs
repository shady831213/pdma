use crate::{
    Error,
    control::resources::{ResourceError, Resources},
};
use pdma_model_contract::control::{
    ControlRequest, ControlRequestOpcode, ControlResponse, ControlResponseStatus,
};
use pdma_model_contract::dma::ChannelId;
use pdma_model_contract::ring::RingError;

impl<
    B: hardware_interface_io::register::RegisterAccess + Sync,
    M: hardware_interface_io::memory::MemoryAccess + Sync,
> super::Service<'_, B, M>
{
    pub(super) async fn dispatch(
        &self,
        (resources, request): (&mut Resources, ControlRequest),
        (monitor, events): (&super::Monitor<'_, B>, &mut super::Events<'_, B, M>),
    ) -> Result<
        (ControlResponse, Option<crate::control::contexts::Pending>),
        Error<B::Error, M::Error>,
    > {
        match request.opcode {
            ControlRequestOpcode::ResetEvents => {
                let mut result = super::response(request);
                result.status = events.reset().await?;
                return Ok((result, None));
            }
            ControlRequestOpcode::AcquireChannel
            | ControlRequestOpcode::ReleaseChannel
            | ControlRequestOpcode::ResetChannel
            | ControlRequestOpcode::ResetDma
            | ControlRequestOpcode::CreateContext
            | ControlRequestOpcode::DrainContext
            | ControlRequestOpcode::AbortContext
            | ControlRequestOpcode::DestroyContext => {}
        }
        let result = self
            .watch(
                self.execute(resources, (monitor, request)),
                (monitor, Some(&mut *events)),
            )
            .await?;
        self.observe((monitor, Some(events))).await?;
        match result {
            Ok((mut response, pending)) => {
                if pending.is_none() && monitor.failed() {
                    response.status = ControlResponseStatus::DeviceFailed;
                }
                Ok((response, pending))
            }
            Err(Error::DeviceFailed) => {
                let mut failed = super::response(request);
                failed.status = ControlResponseStatus::DeviceFailed;
                Ok((failed, None))
            }
            Err(error) => Err(error),
        }
    }
}

impl<B: hardware_interface_io::register::RegisterAccess + Sync, M: Sync> super::Service<'_, B, M> {
    pub(super) async fn execute<E>(
        &self,
        resources: &mut Resources,
        (monitor, request): (&super::Monitor<'_, B>, ControlRequest),
    ) -> Result<(ControlResponse, Option<crate::control::contexts::Pending>), Error<B::Error, E>>
    {
        match request.opcode {
            ControlRequestOpcode::ResetDma => {
                monitor.reset().await?;
                return Ok((super::response(request), None));
            }
            ControlRequestOpcode::ResetEvents => {
                let mut response = super::response(request);
                response.status = ControlResponseStatus::InvalidState;
                return Ok((response, None));
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
                return Ok((response, None));
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
                    let pending = contexts
                        .execute((self.bus, monitor), (&request, &mut response))
                        .await?;
                    return Ok((response, pending));
                }
                None => response.status = ControlResponseStatus::InvalidContext,
            },
        }
        Ok((response, None))
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
