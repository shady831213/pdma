mod channel;
mod resources;

use crate::Error;
use hardware_interface_runtime::{InterruptRegistration, MemoryAccess, RegisterAccess};
use pdma_model_contract::{
    FIRMWARE_STATUS_BASE, FirmwareBootError, FirmwareBootState, FirmwareBootValues,
    WINDOW_CONTROL_BASE,
};
use pdma_model_software::control::{ControlIrq, ControlIsr, InterruptEvent};
use std::{convert::Infallible, future::pending};

/// Boots with DMA access closed, then serves control rings on the firmware CPU.
///
/// Cancelling this program stops software, not accepted DMA work or hardware reset.
/// Retain all ring/application memory until the subsystem is safely stopped.
pub async fn main<B: RegisterAccess + Sync, M: MemoryAccess + Send + Sync>(
    bus: B,
    memory: M,
    registration: InterruptRegistration<'_, ControlIrq, ControlIsr>,
) -> Result<Infallible, Error<B::Error>> {
    let interrupt = InterruptEvent::new();
    registration
        .register((), interrupt.handler())
        .map_err(Error::Registration)?;
    let setup = bus.write(WINDOW_CONTROL_BASE, &0_u32.to_le_bytes()).await;
    let report = match setup {
        Ok(()) => FirmwareBootValues::default().with_state(FirmwareBootState::Ready),
        Err(_) => FirmwareBootValues::default()
            .with_state(FirmwareBootState::Failed)
            .with_error(FirmwareBootError::WindowSetup),
    };
    bus.write(FIRMWARE_STATUS_BASE, &report.raw().to_le_bytes())
        .await
        .map_err(Error::Bus)?;
    match setup {
        Ok(()) => serve(&bus, &memory, &interrupt).await,
        Err(_) => pending().await,
    }
}

async fn serve<B: RegisterAccess + Sync, M: MemoryAccess + Sync>(
    bus: &B,
    memory: &M,
    interrupt: &InterruptEvent,
) -> Result<Infallible, Error<B::Error>> {
    let mut channel = channel::Channel::attach(bus, memory, interrupt).await?;
    let mut allocated = false;
    loop {
        let request = channel.receive().await?;
        let result = resources::execute(bus, &mut allocated, request).await?;
        channel.respond(result).await?;
        tokio::task::yield_now().await;
    }
}
