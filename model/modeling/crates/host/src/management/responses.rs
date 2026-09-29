use super::{Completion, Management, transport::ring_error};
use crate::Error;
use hardware_interface_runtime::{MemoryAccess, RegisterAccess};
use pdma_model_contract::control::{ControlRequestOpcode, ControlResponse};

impl<B: RegisterAccess + Sync, M: MemoryAccess + Send + Sync> Management<B, M> {
    /// Collects one available response without waiting for firmware or an interrupt.
    /// Bus notification may suspend; cancellation retains the response until retried.
    pub async fn try_response(
        &mut self,
    ) -> Result<Option<ControlResponse>, Error<B::Error, M::Error>> {
        self.collect().await
    }

    /// Waits on ISR hints until any outstanding request completes, in response order.
    pub async fn next_response(&mut self) -> Result<ControlResponse, Error<B::Error, M::Error>> {
        loop {
            if let Some(response) = self.collect().await? {
                return Ok(response);
            }
            if self.outstanding.is_empty() {
                return Err(Error::ManagementIdle);
            }
            self.interrupt.wait().await;
        }
    }

    pub(super) async fn collect(
        &mut self,
    ) -> Result<Option<ControlResponse>, Error<B::Error, M::Error>> {
        if self.submission.is_some() {
            return Err(Error::ManagementBusy);
        }
        let mut completion = if let Some(completion) = self.completion {
            completion
        } else {
            let Some(response) = self
                .response
                .peek(&self.layout.response_io(&self.memory))
                .await
                .map_err(ring_error)?
            else {
                return Ok(None);
            };
            let request = self
                .outstanding
                .iter()
                .find(|request| request.id == response.id)
                .ok_or(Error::ManagementProtocol)?;
            match request.opcode {
                ControlRequestOpcode::AcquireChannel
                | ControlRequestOpcode::CreateContext
                | ControlRequestOpcode::ResetDma
                | ControlRequestOpcode::ResetEvents => {}
                ControlRequestOpcode::ReleaseChannel | ControlRequestOpcode::ResetChannel => {
                    if response.channel != request.channel {
                        return Err(Error::ManagementProtocol);
                    }
                }
                ControlRequestOpcode::DrainContext
                | ControlRequestOpcode::AbortContext
                | ControlRequestOpcode::DestroyContext => {
                    if response.context != request.context {
                        return Err(Error::ManagementProtocol);
                    }
                }
            }
            let completion = Completion {
                response,
                consumed: false,
            };
            self.completion = Some(completion);
            completion
        };
        if !completion.consumed {
            self.response
                .consume(&self.layout.response_io(&self.memory))
                .await
                .map_err(ring_error)?;
            completion.consumed = true;
            self.completion = Some(completion);
        }
        self.kick().await?;
        self.outstanding
            .retain(|request| request.id != completion.response.id);
        self.completion = None;
        Ok(Some(completion.response))
    }
}
