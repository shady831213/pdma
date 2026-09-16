//! Device-side management software over injected register access.

use hardware_interface_runtime::{CpuContextError, CpuOwner, RegisterAccess};
use pdma_model_contract::{
    FIRMWARE_STATUS_BASE, FirmwareBootError, FirmwareBootState, FirmwareBootValues,
    WINDOW_CONTROL_BASE,
};
use std::{convert::Infallible, error, fmt, future::pending};

/// Failure to initialize hardware from the firmware software domain.
#[derive(Debug)]
pub enum Error<E> {
    /// Firmware was polled outside its owning software CPU.
    Cpu(CpuContextError),
    /// The management access failed.
    Bus(E),
}

impl<E: fmt::Display> fmt::Display for Error<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cpu(error) => write!(formatter, "firmware CPU: {error}"),
            Self::Bus(error) => write!(formatter, "firmware management access: {error}"),
        }
    }
}

impl<E: error::Error + 'static> error::Error for Error<E> {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Cpu(error) => Some(error),
            Self::Bus(error) => Some(error),
        }
    }
}

/// Initializes the work window and publishes Ready or a reportable boot failure.
///
/// Both outcomes remain idle until simulation shutdown. CPU or report-transport
/// failures propagate to the caller; cancellation does not reset hardware.
pub async fn main<B: RegisterAccess + Sync>(bus: B) -> Result<Infallible, Error<B::Error>> {
    let owner = CpuOwner::capture_active().map_err(Error::Cpu)?;
    let setup = owner
        .scope(bus.write(WINDOW_CONTROL_BASE, &1_u32.to_le_bytes()))
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
    pending().await
}
