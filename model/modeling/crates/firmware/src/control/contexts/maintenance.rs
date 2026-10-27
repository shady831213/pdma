use super::{Contexts, Monitor};
use crate::Error;
use hardware_interface_register::RegisterAccess;
use pdma_model_contract::control::{
    ControlRequest, ControlRequestOpcode, ControlResponse, ControlResponseStatus,
};
use pdma_model_contract::dma::ChannelId;
use pdma_model_contract::hw_components::context::{
    ContextEntryRegisters, ContextManagementRegisters, ContextRequestAction, ContextRequestValues,
    ContextResultState, ContextResultValues,
};
use pdma_model_contract::memory::FunctionId;
use pdma_model_contract::ring::RingError;

pub(in crate::control) struct Pending {
    pub(in crate::control) function: FunctionId,
    entry: usize,
    destroy: bool,
}
impl Contexts {
    pub(in crate::control) async fn execute<B: RegisterAccess + Sync, M>(
        &self,
        (bus, monitor): (&B, &Monitor<'_, B>),
        (request, response): (&ControlRequest, &mut ControlResponse),
    ) -> Result<Option<Pending>, Error<B::Error, M>> {
        let (action, destroy) = match request.opcode {
            ControlRequestOpcode::CreateContext => {
                self.create((bus, monitor), (request, response)).await?;
                return Ok(None);
            }
            ControlRequestOpcode::DrainContext => (ContextRequestAction::Drain, false),
            ControlRequestOpcode::DestroyContext => (ContextRequestAction::Drain, true),
            ControlRequestOpcode::AbortContext => (ContextRequestAction::Abort, false),
            ControlRequestOpcode::AcquireChannel
            | ControlRequestOpcode::ReleaseChannel
            | ControlRequestOpcode::ResetDma
            | ControlRequestOpcode::ResetEvents
            | ControlRequestOpcode::ResetChannel => return Ok(None),
        };
        let index =
            usize::try_from(request.context).map_err(|_| Error::Ring(RingError::Geometry))?;
        if index >= self.count {
            response.status = ControlResponseStatus::InvalidContext;
            return Ok(None);
        }
        let entry = self.entry(index)?;
        if self
            .read(bus, entry + ContextEntryRegisters::VALID.offset())
            .await?
            == 0
        {
            response.status = ControlResponseStatus::InvalidContext;
            return Ok(None);
        }
        if self
            .read(bus, entry + ContextEntryRegisters::OWNER.offset())
            .await?
            != request.function
        {
            response.status = ControlResponseStatus::PermissionDenied;
            return Ok(None);
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
        Ok(Some(Pending {
            function: FunctionId::new(request.function),
            entry,
            destroy,
        }))
    }

    pub(in crate::control) async fn poll<B: RegisterAccess + Sync, M>(
        &self,
        (bus, monitor): (&B, &Monitor<'_, B>),
        (pending, response): (&Pending, &mut ControlResponse),
        channels: impl Iterator<Item = ChannelId> + Send,
    ) -> Result<bool, Error<B::Error, M>> {
        let raw = self
            .read(bus, ContextManagementRegisters::RESULT.offset())
            .await?;
        let state = ContextResultValues::new(raw)
            .state()
            .map_err(Error::RegisterCodec)?;
        response.status = match state {
            ContextResultState::Pending => return Ok(false),
            ContextResultState::Success => {
                if pending.destroy {
                    monitor.healthy().await?;
                    self.write(
                        bus,
                        (pending.entry + ContextEntryRegisters::VALID.offset(), 0),
                    )
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
        Ok(true)
    }
}
