use super::{Management, Pending, transport::ring_error};
use crate::Error;
use hardware_interface_runtime::{MemoryAccess, RegisterAccess};
use pdma_model_contract::{ControlRequest, ControlRequestOpcode, ControlResponse};
use pdma_model_software::ring::SoftRingError;

impl<B: RegisterAccess + Sync, M: MemoryAccess + Send + Sync> Management<B, M> {
    pub(super) async fn begin(
        &mut self,
        opcode: ControlRequestOpcode,
        channel: u32,
    ) -> Result<(), Error<B::Error>> {
        self.owner.validate_active().map_err(Error::Cpu)?;
        if self.pending.is_some() {
            return Err(Error::ManagementBusy);
        }
        let require_empty = |result| match result {
            Ok(true) => Ok(()),
            Ok(false) | Err(SoftRingError::Ring(_)) => Err(Error::ManagementProtocol),
            Err(error) => Err(ring_error(error)),
        };
        require_empty(
            self.command
                .is_empty(&self.layout.command_io(&self.memory))
                .await,
        )?;
        require_empty(
            self.response
                .is_empty(&self.layout.response_io(&self.memory))
                .await,
        )?;
        let id = self.next_id;
        self.next_id = id.checked_add(1).ok_or(Error::ManagementProtocol)?;
        self.pending = Some(Pending {
            request: ControlRequest {
                id,
                opcode,
                channel,
            },
            published: false,
            consumed: false,
            response: None,
        });
        Ok(())
    }

    pub(super) async fn collect(&mut self) -> Result<ControlResponse, Error<B::Error>> {
        let mut pending = self.pending.ok_or(Error::ManagementIdle)?;
        if !pending.published {
            let io = self.layout.command_io(&self.memory);
            if self.command.has_pending_push() {
                self.command.resume_push(&io).await.map_err(ring_error)?;
            } else if !self
                .command
                .try_push(&io, pending.request)
                .await
                .map_err(ring_error)?
            {
                return Err(Error::Full);
            }
            pending.published = true;
            self.pending = Some(pending);
        }
        self.kick().await?;
        let response = match pending.response {
            Some(response) => response,
            None => loop {
                let io = self.layout.response_io(&self.memory);
                if let Some(response) = self.response.peek(&io).await.map_err(ring_error)? {
                    if response.id != pending.request.id {
                        return Err(Error::ManagementProtocol);
                    }
                    pending.response = Some(response);
                    self.pending = Some(pending);
                    break response;
                }
                self.interrupt.wait().await;
            },
        };
        if !pending.consumed {
            let io = self.layout.response_io(&self.memory);
            self.response.consume(&io).await.map_err(ring_error)?;
            pending.consumed = true;
            self.pending = Some(pending);
        }
        self.kick().await?;
        self.pending = None;
        Ok(response)
    }
}
