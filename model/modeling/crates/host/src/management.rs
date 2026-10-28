mod contexts;
mod events;
mod pending;
mod responses;
mod setup;
mod transport;

use crate::{Error, RingConfig};
use hw_model_io::memory::{MemoryAccess, MemoryAddress};
use hw_model_io::register::RegisterAccess;
use pdma_model_contract::control::{
    ControlEvent, ControlRequest, ControlRequestOpcode, ControlResponse, ControlResponseStatus,
};
use pdma_model_contract::dma::ChannelId;
use pdma_model_contract::memory::FunctionId;
use pdma_model_software::control::{ControlQueueLayout, InterruptEvent, ManagementAddresses};
use pdma_model_software::ring::SoftRing;

/// Host-owned storage for the independent management queues.
#[derive(Debug, Clone, Copy)]
pub struct ManagementRings {
    /// Shared queue descriptor, kept alive with all three entry buffers.
    pub state: MemoryAddress,
    /// Host-produced firmware requests.
    pub command: RingConfig<ControlRequest>,
    /// Firmware-produced management results.
    pub response: RingConfig<ControlResponse>,
    /// Firmware-produced asynchronous system events.
    pub event: RingConfig<ControlEvent>,
}

/// Control storage and the work event served by the program's registered ISR.
#[derive(Debug)]
pub struct ManagementConfig {
    /// Register placement supplied by the integration platform.
    pub addresses: ManagementAddresses,
    /// Caller-owned control queue storage.
    pub rings: ManagementRings,
    /// Register its handler before enabling the control channel.
    pub interrupt: InterruptEvent,
    /// Independent event-doorbell ISR wake hint.
    pub event_interrupt: InterruptEvent,
}

#[derive(Debug, Clone, Copy)]
struct Submission {
    request: ControlRequest,
    published: bool,
}

#[derive(Debug, Clone, Copy)]
struct Completion {
    consumed: bool,
    response: ControlResponse,
}

/// Correlation identity allocated when a management submission is accepted.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ManagementRequestId(u64);

impl ManagementRequestId {
    /// Returns the identity encoded in the command and response records.
    pub const fn raw(self) -> u64 {
        self.0
    }
}

/// Host endpoint with command-capacity-bounded outstanding requests.
///
/// Keep the client and its backing memory alive across cancelled waits. A timeout
/// does not retract a request; resume incomplete publication with `resume_submission`.
#[derive(Debug)]
pub struct Management<B, M> {
    addresses: ManagementAddresses,
    bus: B,
    memory: M,
    layout: ControlQueueLayout,
    interrupt: InterruptEvent,
    event_interrupt: InterruptEvent,
    event_state: MemoryAddress,
    event: SoftRing<ControlEvent>,
    command: SoftRing<ControlRequest>,
    response: SoftRing<ControlResponse>,
    submission: Option<Submission>,
    completion: Option<Completion>,
    outstanding: Vec<ControlRequest>,
    capacity: usize,
    next_id: u64,
}

impl<B: RegisterAccess + Sync, M: MemoryAccess + Send + Sync> Management<B, M> {
    /// Requests a clean channel; firmware chooses its physical identifier.
    pub async fn acquire_channel(
        &mut self,
        function: FunctionId,
    ) -> Result<ChannelId, Error<B::Error, M::Error>> {
        self.require_idle()?;
        self.submit(ControlRequestOpcode::AcquireChannel, function, 0)
            .await?;
        let response = self.wait_pending().await?;
        match response.status {
            ControlResponseStatus::Success => {}
            status @ (ControlResponseStatus::NoResources
            | ControlResponseStatus::InvalidChannel
            | ControlResponseStatus::NotAllocated
            | ControlResponseStatus::PermissionDenied
            | ControlResponseStatus::InvalidFunction
            | ControlResponseStatus::ContextBusy
            | ControlResponseStatus::InvalidContext
            | ControlResponseStatus::MaintenanceFailed
            | ControlResponseStatus::InvalidAttributes
            | ControlResponseStatus::DeviceFailed
            | ControlResponseStatus::InvalidState) => {
                return Err(Error::ManagementStatus(status));
            }
        }
        Ok(ChannelId::new(
            usize::try_from(response.channel).map_err(|_| Error::Geometry)?,
        ))
    }

    /// Closes access and waits for firmware to observe completed channel reset.
    ///
    /// Stop software ring/ISR use first. Success invalidates the old channel driver;
    /// application memory must remain valid until this response is collected.
    pub async fn release_channel(
        &mut self,
        function: FunctionId,
        channel: ChannelId,
    ) -> Result<(), Error<B::Error, M::Error>> {
        self.require_idle()?;
        self.submit(
            ControlRequestOpcode::ReleaseChannel,
            function,
            u32::try_from(channel.index()).map_err(|_| Error::Geometry)?,
        )
        .await?;
        let response = self.wait_pending().await?;
        match response.status {
            ControlResponseStatus::Success => Ok(()),
            status @ (ControlResponseStatus::NoResources
            | ControlResponseStatus::InvalidChannel
            | ControlResponseStatus::NotAllocated
            | ControlResponseStatus::PermissionDenied
            | ControlResponseStatus::InvalidFunction
            | ControlResponseStatus::ContextBusy
            | ControlResponseStatus::InvalidContext
            | ControlResponseStatus::MaintenanceFailed
            | ControlResponseStatus::InvalidAttributes
            | ControlResponseStatus::DeviceFailed
            | ControlResponseStatus::InvalidState) => Err(Error::ManagementStatus(status)),
        }
    }

    /// Resets an allocated channel, retaining its grant after hardware cleanup.
    /// Stop ring and ISR use first; after success, configure a fresh DMA driver.
    pub async fn reset_channel(
        &mut self,
        function: FunctionId,
        channel: ChannelId,
    ) -> Result<(), Error<B::Error, M::Error>> {
        self.require_idle()?;
        self.submit(
            ControlRequestOpcode::ResetChannel,
            function,
            u32::try_from(channel.index()).map_err(|_| Error::Geometry)?,
        )
        .await?;
        match self.wait_pending().await?.status {
            ControlResponseStatus::Success => Ok(()),
            status @ (ControlResponseStatus::NoResources
            | ControlResponseStatus::InvalidChannel
            | ControlResponseStatus::NotAllocated
            | ControlResponseStatus::PermissionDenied
            | ControlResponseStatus::InvalidFunction
            | ControlResponseStatus::ContextBusy
            | ControlResponseStatus::InvalidContext
            | ControlResponseStatus::MaintenanceFailed
            | ControlResponseStatus::InvalidAttributes
            | ControlResponseStatus::DeviceFailed
            | ControlResponseStatus::InvalidState) => Err(Error::ManagementStatus(status)),
        }
    }

    /// Resumes publication or collection of the same request after cancellation.
    pub async fn wait_pending(&mut self) -> Result<ControlResponse, Error<B::Error, M::Error>> {
        if self.outstanding.is_empty() {
            return Err(Error::ManagementIdle);
        }
        if self.submission.is_some() {
            self.publish().await?;
        }
        self.next_response().await
    }
}
