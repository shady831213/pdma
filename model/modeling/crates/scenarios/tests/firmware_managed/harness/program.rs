//! The complete Host program, including preboot access probes.

pub mod transfer;

use super::runtime::{AccessError, HostBus};
use hardware_interface_runtime::{
    InterruptRegistration, MemoryAddress, MemoryAlignment, RegisterAccess, SharedMemoryAccess,
    SharedMemoryAllocator,
};
use pdma_model_contract::{
    ControlQueue, ControlRequest, ControlResponse, Response, WINDOW_CONTROL_BASE,
    WINDOW_STATUS_BASE,
};
use pdma_model_host::{Management, ManagementRings, RingConfig, firmware, runtime::Irq};
use std::time::Duration;

use super::TestResult;

/// The case-selected Host operation after firmware readiness.
#[derive(Clone, Copy, Debug)]
pub enum Input<'a> {
    /// Copy these bytes and wait for the interrupt handler's response.
    Copy(&'a [u8]),
    /// Exit with an intentional error before allocating application memory.
    StopAfterReady,
}

pub(super) struct Ports<'a> {
    pub(super) bus: HostBus<SharedMemoryAccess>,
    pub(super) memory: SharedMemoryAccess,
    pub(super) allocator: SharedMemoryAllocator,
    pub(super) interrupts: InterruptRegistration<'a, Irq, transfer::Handler>,
}

/// Host observations collected before polling firmware execution.
#[derive(Debug)]
pub struct Before {
    pub(crate) read: Result<(), AccessError>,
    pub(crate) write: Result<(), AccessError>,
    pub(crate) control_read: Result<(), AccessError>,
    pub(crate) control_write: Result<(), AccessError>,
    pub(crate) output: [u8; 4],
    pub(crate) ready: u32,
}

/// Completed transfer and permission observations for case-owned assertions.
#[derive(Debug)]
pub struct Outcome {
    pub(crate) before: Before,
    pub(crate) after: [Result<(), AccessError>; 3],
    pub(crate) response: Response,
    pub(crate) destination: MemoryAddress,
    pub(crate) released: u32,
}

pub(super) async fn main(ports: Ports<'_>, input: Input<'_>) -> TestResult<Outcome> {
    let mut output = [0xa5; 4];
    let mut ready = [0; 4];
    let read = ports.bus.read(0, &mut output).await;
    let write = ports.bus.write(4, &1_u32.to_le_bytes()).await;
    let control_read = ports.bus.read(WINDOW_CONTROL_BASE, &mut [0; 4]).await;
    let control_write = ports
        .bus
        .write(WINDOW_CONTROL_BASE, &1_u32.to_le_bytes())
        .await;
    ports.bus.read(WINDOW_STATUS_BASE, &mut ready).await?;
    let before = Before {
        read,
        write,
        control_read,
        control_write,
        output,
        ready: u32::from_le_bytes(ready),
    };
    firmware::wait_ready(&ports.bus, Duration::from_secs(1)).await?;
    ports.bus.read(WINDOW_STATUS_BASE, &mut ready).await?;
    if u32::from_le_bytes(ready) != 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "firmware readiness unexpectedly granted DMA access",
        )
        .into());
    }
    let payload = match input {
        Input::Copy(payload) => payload,
        Input::StopAfterReady => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "Host stopped after firmware readiness",
            )
            .into());
        }
    };
    let command = ports.allocator.allocate(
        2 * u64::try_from(ControlRequest::RECORD_SIZE)?,
        MemoryAlignment::try_new(u64::try_from(ControlRequest::RECORD_ALIGNMENT)?)?,
    )?;
    let response = ports.allocator.allocate(
        2 * u64::try_from(ControlResponse::RECORD_SIZE)?,
        MemoryAlignment::try_new(u64::try_from(ControlResponse::RECORD_ALIGNMENT)?)?,
    )?;
    let mut management = Management::setup(
        ports.bus.clone(),
        ports.memory.clone(),
        ManagementRings {
            state: ports.allocator.allocate(
                u64::try_from(ControlQueue::RECORD_SIZE)?,
                MemoryAlignment::try_new(u64::try_from(ControlQueue::RECORD_ALIGNMENT)?)?,
            )?,
            command: RingConfig::new(command.raw(), 2)?,
            response: RingConfig::new(response.raw(), 2)?,
        },
    )
    .await?;
    let channel = management.acquire_channel().await?;
    let after = [
        ports
            .bus
            .write(WINDOW_CONTROL_BASE, &0_u32.to_le_bytes())
            .await,
        ports
            .bus
            .write(WINDOW_CONTROL_BASE, &1_u32.to_le_bytes())
            .await,
        ports
            .bus
            .write(WINDOW_STATUS_BASE, &0_u32.to_le_bytes())
            .await,
    ];
    let bus = ports.bus.clone();
    let (response, destination) = transfer::run(ports, payload, channel).await?;
    management.release_channel(channel).await?;
    bus.read(WINDOW_STATUS_BASE, &mut ready).await?;
    Ok(Outcome {
        before,
        after,
        response,
        destination,
        released: u32::from_le_bytes(ready),
    })
}
