mod pending;
mod transport;

use crate::{Error, RingConfig};
use hardware_interface_runtime::{CpuOwner, MemoryAccess, RegisterAccess};
use pdma_model_contract::{
    ChannelId, ControlRegisters, ControlRequest, ControlRequestOpcode, ControlResponse,
    ControlResponseStatus, ControlRingRegisters, Ring,
};

/// Host-owned storage for the independent management queues.
#[derive(Debug, Clone, Copy)]
pub struct ManagementRings {
    /// Host-produced firmware requests.
    pub command: RingConfig<ControlRequest>,
    /// Firmware-produced management results.
    pub response: RingConfig<ControlResponse>,
}

#[derive(Debug, Clone, Copy)]
struct Pending {
    request: ControlRequest,
    producer: u32,
    consumer: u32,
    response: Option<ControlResponse>,
}

/// CPU-local, single-outstanding Host endpoint of the management channel.
///
/// Keep the client and its backing memory alive across cancelled waits. A timeout
/// does not retract a request; resume it with `wait_pending` before submitting again.
#[derive(Debug)]
pub struct Management<B, M> {
    bus: B,
    memory: M,
    owner: CpuOwner,
    rings: ManagementRings,
    pending: Option<Pending>,
    next_id: u64,
}

impl<B: RegisterAccess + Sync, M: MemoryAccess + Send + Sync> Management<B, M> {
    /// Configures empty control rings after firmware Ready, before any DMA grant.
    ///
    /// Geometry becomes immutable on enable. Interrupted setup may leave committed
    /// configuration: retain ring storage until the subsystem is stopped/reset.
    pub async fn setup(bus: B, memory: M, rings: ManagementRings) -> Result<Self, Error<B::Error>> {
        let client = Self {
            bus,
            memory,
            rings,
            owner: CpuOwner::capture_active().map_err(Error::Cpu)?,
            pending: None,
            next_id: 1,
        };
        for (offset, base, capacity) in [
            (
                ControlRegisters::COMMAND.offset(),
                rings.command.base(),
                rings.command.capacity(),
            ),
            (
                ControlRegisters::RESPONSE.offset(),
                rings.response.base(),
                rings.response.capacity(),
            ),
        ] {
            client
                .write(
                    offset + ControlRingRegisters::BASE.offset(),
                    &base.to_le_bytes(),
                )
                .await?;
            client
                .write(
                    offset + ControlRingRegisters::CAPACITY.offset(),
                    &capacity.to_le_bytes(),
                )
                .await?;
        }
        client
            .write(ControlRegisters::ENABLE.offset(), &1_u32.to_le_bytes())
            .await?;
        Ok(client)
    }

    /// Requests a clean channel; firmware chooses its physical identifier.
    pub async fn acquire_channel(&mut self) -> Result<ChannelId, Error<B::Error>> {
        self.begin(ControlRequestOpcode::AcquireChannel, 0).await?;
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
        self.begin(
            ControlRequestOpcode::ReleaseChannel,
            u32::try_from(channel.index()).map_err(|_| Error::Geometry)?,
        )
        .await?;
        let response = self.wait_pending().await?;
        match response.status {
            ControlResponseStatus::Success => Ok(()),
            status @ (ControlResponseStatus::NoResources
            | ControlResponseStatus::InvalidChannel
            | ControlResponseStatus::NotAllocated) => Err(Error::ManagementStatus(status)),
        }
    }

    /// Resumes publication or collection of the same request after cancellation.
    pub async fn wait_pending(&mut self) -> Result<ControlResponse, Error<B::Error>> {
        let owner = self.owner.clone();
        owner.scope(self.collect()).await.map_err(Error::Cpu)?
    }
}
