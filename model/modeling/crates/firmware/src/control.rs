mod resources;
mod rings;

use crate::Error;
use hardware_interface_runtime::{CpuOwner, MemoryAccess, NotificationWait, RegisterAccess};
use pdma_model_contract::{
    ControlQueue, ControlRegisters, FIRMWARE_STATUS_BASE, FirmwareBootError, FirmwareBootState,
    FirmwareBootValues, WINDOW_CONTROL_BASE,
};
use std::{convert::Infallible, future::pending};

/// Boots with DMA access closed, then serves control rings on the firmware CPU.
///
/// Cancelling this program stops software, not accepted DMA work or hardware reset.
/// Retain all ring/application memory until the subsystem is safely stopped.
pub async fn main<B: RegisterAccess + NotificationWait + Sync, M: MemoryAccess + Send + Sync>(
    bus: B,
    memory: M,
) -> Result<Infallible, Error<B::Error>> {
    let owner = CpuOwner::capture_active().map_err(Error::Cpu)?;
    let setup = owner
        .scope(bus.write(WINDOW_CONTROL_BASE, &0_u32.to_le_bytes()))
        .await
        .map_err(Error::Cpu)?;
    let report = match setup {
        Ok(()) => FirmwareBootValues::default().with_state(FirmwareBootState::Ready),
        Err(_) => FirmwareBootValues::default()
            .with_state(FirmwareBootState::Failed)
            .with_error(FirmwareBootError::WindowSetup),
    };
    owner
        .scope(bus.write(FIRMWARE_STATUS_BASE, &report.raw().to_le_bytes()))
        .await
        .map_err(Error::Cpu)?
        .map_err(Error::Bus)?;
    match setup {
        Ok(()) => owner
            .scope(serve(&bus, &memory))
            .await
            .map_err(Error::Cpu)?,
        Err(_) => pending().await,
    }
}

async fn serve<B: RegisterAccess + NotificationWait + Sync, M: MemoryAccess + Sync>(
    bus: &B,
    memory: &M,
) -> Result<Infallible, Error<B::Error>> {
    while rings::read_word(bus, ControlRegisters::ENABLE.offset()).await? == 0 {
        bus.wait_notification().await;
    }
    let mut queues = rings::Queues::attach(bus, memory).await?;
    let command_io = queues
        .io(memory, ControlQueue::COMMAND_OFFSET)
        .map_err(Error::Ring)?;
    let response_io = queues
        .io(memory, ControlQueue::RESPONSE_OFFSET)
        .map_err(Error::Ring)?;
    let mut allocated = false;
    loop {
        if !queues
            .response
            .has_space(&response_io)
            .await
            .map_err(Error::SoftRing)?
        {
            bus.wait_notification().await;
            continue;
        }
        let Some(request) = queues
            .command
            .peek(&command_io)
            .await
            .map_err(Error::SoftRing)?
        else {
            bus.wait_notification().await;
            continue;
        };
        queues
            .command
            .consume(&command_io)
            .await
            .map_err(Error::SoftRing)?;
        rings::notify(bus).await?;
        let result = resources::execute(bus, &mut allocated, request).await?;
        while !queues
            .response
            .try_push(&response_io, result)
            .await
            .map_err(Error::SoftRing)?
        {
            bus.wait_notification().await;
        }
        rings::notify(bus).await?;
        tokio::task::yield_now().await;
    }
}
