mod channel;
mod resources;

use crate::Error;
use hardware_interface_runtime::{InterruptRegistration, MemoryAccess, RegisterAccess};
use pdma_model_contract::{
    FirmwareBootError, FirmwareBootState, FirmwareBootValues, FirmwareRegisters, RingError,
};
use pdma_model_software::control::{ControlIrq, ControlIsr, InterruptEvent, ManagementAddresses};
use std::{convert::Infallible, future::pending};

/// Boots with DMA access closed, then serves control rings on the firmware CPU.
///
/// Cancelling this program stops software, not accepted DMA work or hardware reset.
/// Retain all ring/application memory until the subsystem is safely stopped.
pub async fn main<B: RegisterAccess + Sync, M: MemoryAccess + Send + Sync>(
    bus: B,
    memory: M,
    registration: InterruptRegistration<'_, ControlIrq, ControlIsr>,
    addresses: ManagementAddresses,
) -> Result<Infallible, Error<B::Error>> {
    let interrupt = InterruptEvent::new();
    registration
        .register((), interrupt.handler())
        .map_err(Error::Registration)?;
    let setup = resources::Resources::initialize(&bus, addresses).await;
    let report = match &setup {
        Ok(_) => FirmwareBootValues::default().with_state(FirmwareBootState::Ready),
        Err(_) => FirmwareBootValues::default()
            .with_state(FirmwareBootState::Failed)
            .with_error(FirmwareBootError::WindowSetup),
    };
    bus.write(
        addresses
            .firmware
            .checked_add(
                u32::try_from(FirmwareRegisters::BOOT.offset())
                    .map_err(|_| Error::Ring(RingError::Geometry))?,
            )
            .ok_or(Error::Ring(RingError::Geometry))?,
        &report.raw().to_le_bytes(),
    )
    .await
    .map_err(Error::Bus)?;
    match setup {
        Ok(resources) => {
            resources
                .serve(&bus, &memory, &interrupt, addresses.control)
                .await
        }
        Err(_) => pending().await,
    }
}
