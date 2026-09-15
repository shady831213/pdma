//! The complete Host program, including preboot access probes.

pub mod transfer;

use hardware_interface_runtime::{
    InterruptRegistration, MemoryAddress, RegisterAccess, SharedMemoryAccess, SharedMemoryAllocator,
};
use pdma_model_contract::{Response, WINDOW_CONTROL_BASE, WINDOW_STATUS_BASE};
use pdma_model_host::runtime::Irq;
use pdma_model_managed_runtime::{AccessError, HostBus};
use tokio::sync::oneshot;

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
}

pub(super) async fn main(
    ports: Ports<'_>,
    input: Input<'_>,
    start: oneshot::Sender<()>,
) -> TestResult<Outcome> {
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
    start
        .send(())
        .map_err(|()| std::io::Error::other("system startup receiver closed"))?;
    loop {
        ports.bus.read(WINDOW_STATUS_BASE, &mut ready).await?;
        if u32::from_le_bytes(ready) == 1 {
            break;
        }
        tokio::task::yield_now().await;
    }
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
    let (response, destination) = transfer::run(ports, payload).await?;
    Ok(Outcome {
        before,
        after,
        response,
        destination,
    })
}
