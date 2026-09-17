use super::{
    Management, Pending,
    transport::{ring_error, ring_io},
};
use crate::Error;
use hardware_interface_runtime::{MemoryAccess, NotificationWait, RegisterAccess};
use pdma_model_contract::{ControlQueue, ControlRequest, ControlRequestOpcode, ControlResponse};
use pdma_model_software::ring::SoftRingIo;

impl<B: RegisterAccess + NotificationWait, M: MemoryAccess + Send + Sync> Management<B, M> {
    pub(super) async fn begin(
        &mut self,
        opcode: ControlRequestOpcode,
        channel: u32,
    ) -> Result<(), Error<B::Error>> {
        self.owner.validate_active().map_err(Error::Cpu)?;
        if self.pending.is_some() {
            return Err(Error::ManagementBusy);
        }
        for offset in [ControlQueue::COMMAND_OFFSET, ControlQueue::RESPONSE_OFFSET] {
            let io = ring_io(&self.memory, self.rings.state, offset)?;
            let producer = io.read_producer().await.map_err(Error::Memory)?;
            let consumer = io.read_consumer().await.map_err(Error::Memory)?;
            if producer != consumer {
                return Err(Error::ManagementProtocol);
            }
        }
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
            let io = ring_io(&self.memory, self.rings.state, ControlQueue::COMMAND_OFFSET)?;
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
                let io = ring_io(
                    &self.memory,
                    self.rings.state,
                    ControlQueue::RESPONSE_OFFSET,
                )?;
                if let Some(response) = self.response.peek(&io).await.map_err(ring_error)? {
                    if response.id != pending.request.id {
                        return Err(Error::ManagementProtocol);
                    }
                    pending.response = Some(response);
                    self.pending = Some(pending);
                    break response;
                }
                self.bus.wait_notification().await;
            },
        };
        if !pending.consumed {
            let io = ring_io(
                &self.memory,
                self.rings.state,
                ControlQueue::RESPONSE_OFFSET,
            )?;
            self.response.consume(&io).await.map_err(ring_error)?;
            pending.consumed = true;
            self.pending = Some(pending);
        }
        self.kick().await?;
        self.pending = None;
        Ok(response)
    }
}
