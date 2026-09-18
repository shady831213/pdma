//! Host observation of the firmware boot lifecycle.

use crate::Error;
use hardware_interface_runtime::{CpuOwner, RegisterAccess};
use pdma_model_contract::{FirmwareBootState, FirmwareBootValues, FirmwareRegisters};
use std::time::Duration;

/// Waits for firmware initialization within a software-owned deadline.
///
/// Ready does not grant DMA access. Cancellation or timeout changes no hardware
/// state and does not stop firmware. The caller must run on a software CPU.
pub async fn wait_ready<B: RegisterAccess + Sync>(
    bus: &B,
    base: u32,
    timeout: Duration,
) -> Result<(), Error<B::Error>> {
    let owner = CpuOwner::capture_active().map_err(Error::Cpu)?;
    owner
        .scope(tokio::time::timeout(timeout, async {
            loop {
                let mut bytes = [0; 4];
                bus.read(
                    base.checked_add(
                        u32::try_from(FirmwareRegisters::BOOT.offset())
                            .map_err(|_| Error::Geometry)?,
                    )
                    .ok_or(Error::Geometry)?,
                    &mut bytes,
                )
                .await
                .map_err(Error::Bus)?;
                let status = FirmwareBootValues::new(u32::from_le_bytes(bytes));
                match status.state().map_err(Error::RegisterCodec)? {
                    FirmwareBootState::Ready => return Ok(()),
                    FirmwareBootState::Failed => {
                        return Err(Error::FirmwareFailed(
                            status.error().map_err(Error::RegisterCodec)?,
                        ));
                    }
                    FirmwareBootState::Booting => tokio::task::yield_now().await,
                }
            }
        }))
        .await
        .map_err(Error::Cpu)?
        .map_err(|_| Error::FirmwareTimeout)?
}
