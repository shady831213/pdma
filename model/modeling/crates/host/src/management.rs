use pdma_model_software::control::{ControlQueueLayout, InterruptEvent, ManagementAddresses};
use pdma_model_software::ring::SoftRing;
mod pending;
mod responses;
mod transport;

use crate::{Error, RingConfig};
use hardware_interface_runtime::{
    CpuOwner, MemoryAccess, MemoryAddress, MemoryAlignment, RegisterAccess,
};
use hardware_interface_schema::Encode;
use pdma_model_contract::{
    ChannelId, ControlQueue, ControlRegisters, ControlRequest, ControlResponse,
    ControlResponseStatus, ControlRingState, Ring,
};

/// Host-owned storage for the independent management queues.
#[derive(Debug, Clone, Copy)]
pub struct ManagementRings {
    /// Shared queue descriptor, kept alive with both entry buffers.
    pub state: MemoryAddress,
    /// Host-produced firmware requests.
    pub command: RingConfig<ControlRequest>,
    /// Firmware-produced management results.
    pub response: RingConfig<ControlResponse>,
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

/// CPU-local Host endpoint with command-capacity-bounded outstanding requests.
///
/// Keep the client and its backing memory alive across cancelled waits. A timeout
/// does not retract a request; resume incomplete publication with `resume_submission`.
#[derive(Debug)]
pub struct Management<B, M> {
    addresses: ManagementAddresses,
    bus: B,
    memory: M,
    owner: CpuOwner,
    layout: ControlQueueLayout,
    interrupt: InterruptEvent,
    command: SoftRing<ControlRequest>,
    response: SoftRing<ControlResponse>,
    submission: Option<Submission>,
    completion: Option<Completion>,
    outstanding: Vec<ControlRequest>,
    capacity: usize,
    next_id: u64,
}

impl<B: RegisterAccess + Sync, M: MemoryAccess + Send + Sync> Management<B, M> {
    /// Configures empty control rings after firmware Ready, before any DMA grant.
    ///
    /// Geometry becomes immutable on enable. Interrupted setup may leave committed
    /// configuration: retain ring storage until the subsystem is stopped/reset.
    pub async fn setup(
        bus: B,
        memory: M,
        config: ManagementConfig,
    ) -> Result<Self, Error<B::Error>> {
        let ManagementConfig {
            rings,
            interrupt,
            addresses,
        } = config;
        let client = Self {
            addresses,
            bus,
            memory,
            layout: ControlQueueLayout::new(rings.state).map_err(Error::Ring)?,
            interrupt,
            command: SoftRing::new(rings.command).map_err(Error::Cpu)?,
            response: SoftRing::new(rings.response).map_err(Error::Cpu)?,
            owner: CpuOwner::capture_active().map_err(Error::Cpu)?,
            submission: None,
            completion: None,
            outstanding: Vec::new(),
            capacity: usize::try_from(rings.command.capacity()).map_err(|_| Error::Geometry)?,
            next_id: 1,
        };
        if client.read(ControlRegisters::ENABLE.offset()).await? != 0 {
            return Err(Error::ManagementBusy);
        }
        let state = ControlQueue {
            command: ControlRingState {
                base: rings.command.base(),
                capacity: rings.command.capacity(),
                producer: 0,
                consumer: 0,
            },
            response: ControlRingState {
                base: rings.response.base(),
                capacity: rings.response.capacity(),
                producer: 0,
                consumer: 0,
            },
        };
        client
            .memory
            .write_typed(
                rings.state,
                MemoryAlignment::try_new(
                    u64::try_from(ControlQueue::RECORD_ALIGNMENT).map_err(|_| Error::Geometry)?,
                )
                .map_err(|_| Error::Geometry)?,
                &state.encode().map_err(Error::Codec)?,
            )
            .map_err(Error::Memory)?;
        client
            .write(
                ControlRegisters::BOOTSTRAP.offset(),
                &rings.state.raw().to_le_bytes(),
            )
            .await?;
        client
            .write(ControlRegisters::ENABLE.offset(), &1_u32.to_le_bytes())
            .await?;
        client.kick().await?;
        Ok(client)
    }

    /// Requests a clean channel; firmware chooses its physical identifier.
    pub async fn acquire_channel(&mut self) -> Result<ChannelId, Error<B::Error>> {
        self.require_idle()?;
        self.submit_acquire().await?;
        let response = self.wait_pending().await?;
        match response.status {
            ControlResponseStatus::Success => {}
            status @ (ControlResponseStatus::NoResources
            | ControlResponseStatus::InvalidChannel
            | ControlResponseStatus::NotAllocated) => return Err(Error::ManagementStatus(status)),
        }
        Ok(ChannelId::new(
            usize::try_from(response.channel).map_err(|_| Error::Geometry)?,
        ))
    }

    /// Closes access and waits for firmware to observe completed channel reset.
    ///
    /// Stop software ring/ISR use first. Success invalidates the old channel driver;
    /// application memory must remain valid until this response is collected.
    pub async fn release_channel(&mut self, channel: ChannelId) -> Result<(), Error<B::Error>> {
        self.require_idle()?;
        self.submit_release(channel).await?;
        let response = self.wait_pending().await?;
        match response.status {
            ControlResponseStatus::Success => Ok(()),
            status @ (ControlResponseStatus::NoResources
            | ControlResponseStatus::InvalidChannel
            | ControlResponseStatus::NotAllocated) => Err(Error::ManagementStatus(status)),
        }
    }

    /// Resets an allocated channel, retaining its grant after hardware cleanup.
    /// Stop ring and ISR use first; after success, configure a fresh DMA driver.
    pub async fn reset_channel(&mut self, channel: ChannelId) -> Result<(), Error<B::Error>> {
        self.require_idle()?;
        self.submit_reset(channel).await?;
        match self.wait_pending().await?.status {
            ControlResponseStatus::Success => Ok(()),
            status @ (ControlResponseStatus::NoResources
            | ControlResponseStatus::InvalidChannel
            | ControlResponseStatus::NotAllocated) => Err(Error::ManagementStatus(status)),
        }
    }

    /// Resumes publication or collection of the same request after cancellation.
    pub async fn wait_pending(&mut self) -> Result<ControlResponse, Error<B::Error>> {
        self.owner.validate_active().map_err(Error::Cpu)?;
        if self.outstanding.is_empty() {
            return Err(Error::ManagementIdle);
        }
        if self.submission.is_some() {
            self.resume_submission().await?;
        }
        self.next_response().await
    }
}
