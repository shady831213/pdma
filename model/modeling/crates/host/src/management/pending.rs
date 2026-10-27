use super::{Management, ManagementRequestId, Submission, transport::ring_error};
use crate::Error;
use hardware_interface_io::memory::MemoryAccess;
use hardware_interface_io::register::RegisterAccess;
use pdma_model_contract::control::{ControlRequest, ControlRequestOpcode};
use pdma_model_contract::dma::ChannelId;
use pdma_model_contract::memory::FunctionId;
use pdma_model_software::ring::SoftRingError;

impl<B: RegisterAccess + Sync, M: MemoryAccess + Send + Sync> Management<B, M> {
    /// Publishes an acquisition and rings the doorbell without waiting for firmware.
    /// Cancellation retains its identity; call `resume_submission` before submitting again.
    pub async fn submit_acquire(
        &mut self,
        function: FunctionId,
    ) -> Result<ManagementRequestId, Error<B::Error, M::Error>> {
        self.submit(ControlRequestOpcode::AcquireChannel, function, 0)
            .await
    }

    /// Publishes a release without waiting for reset completion or firmware's response.
    /// Retain channel memory until its successful response has been collected.
    pub async fn submit_release(
        &mut self,
        function: FunctionId,
        channel: ChannelId,
    ) -> Result<ManagementRequestId, Error<B::Error, M::Error>> {
        self.submit(
            ControlRequestOpcode::ReleaseChannel,
            function,
            u32::try_from(channel.index()).map_err(|_| Error::Geometry)?,
        )
        .await
    }

    /// Publishes reset while retaining allocation; keep channel memory until its response.
    pub async fn submit_reset(
        &mut self,
        function: FunctionId,
        channel: ChannelId,
    ) -> Result<ManagementRequestId, Error<B::Error, M::Error>> {
        self.submit(
            ControlRequestOpcode::ResetChannel,
            function,
            u32::try_from(channel.index()).map_err(|_| Error::Geometry)?,
        )
        .await
    }

    /// Finishes the retained publication and notification, returning the same identity.
    pub async fn resume_submission(
        &mut self,
    ) -> Result<ManagementRequestId, Error<B::Error, M::Error>> {
        self.publish().await
    }

    pub(super) const fn require_idle(&self) -> Result<(), Error<B::Error, M::Error>> {
        if !self.outstanding.is_empty() {
            return Err(Error::ManagementBusy);
        }
        Ok(())
    }

    pub(super) async fn submit(
        &mut self,
        opcode: ControlRequestOpcode,
        function: FunctionId,
        channel: u32,
    ) -> Result<ManagementRequestId, Error<B::Error, M::Error>> {
        self.submit_request(ControlRequest {
            id: 0,
            opcode,
            function: function.raw(),
            channel,
            context: 0,
            attributes: pdma_model_contract::memory::ContextAttributes::DEFAULT,
        })
        .await
    }

    pub(super) async fn submit_request(
        &mut self,
        mut request: ControlRequest,
    ) -> Result<ManagementRequestId, Error<B::Error, M::Error>> {
        if self.submission.is_some() {
            return Err(Error::ManagementBusy);
        }
        if self.outstanding.len() == self.capacity {
            return Err(Error::Full);
        }
        if self.outstanding.is_empty() {
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
        }
        if !self
            .command
            .has_space(&self.layout.command_io(&self.memory))
            .await
            .map_err(ring_error)?
        {
            return Err(Error::Full);
        }
        let id = self.next_id;
        self.next_id = id.checked_add(1).ok_or(Error::ManagementProtocol)?;
        request.id = id;
        self.outstanding.push(request);
        self.submission = Some(Submission {
            request,
            published: false,
        });
        self.publish().await
    }

    pub(super) async fn publish(
        &mut self,
    ) -> Result<ManagementRequestId, Error<B::Error, M::Error>> {
        let mut submission = self.submission.ok_or(Error::ManagementIdle)?;
        if !submission.published {
            let io = self.layout.command_io(&self.memory);
            if self.command.has_pending_push() {
                self.command.resume_push(&io).await.map_err(ring_error)?;
            } else if !self
                .command
                .try_push(&io, submission.request)
                .await
                .map_err(ring_error)?
            {
                return Err(Error::Full);
            }
            submission.published = true;
            self.submission = Some(submission);
        }
        self.kick().await?;
        self.submission = None;
        Ok(ManagementRequestId(submission.request.id))
    }
}
