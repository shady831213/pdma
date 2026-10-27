use hardware_interface_pipeline::AsyncRunError;
use std::{convert::Infallible, fmt};

mod access;
pub use access::AccessError;

/// Invalid scenario-platform configuration, independent of any hardware block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformConfigError {
    /// The Function namespace is empty or cannot be represented.
    FunctionCount,
    /// Optional context capacity and register base must be supplied together.
    ContextConfiguration,
    /// A generated register population has invalid geometry.
    Layout(hardware_interface_schema::RegisterBankError),
    /// A block is misaligned or exceeds the platform's address representation.
    AddressSpace,
    /// Two platform register apertures overlap.
    Overlap,
}

impl fmt::Display for PlatformConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FunctionCount => f.write_str("invalid platform Function count"),
            Self::ContextConfiguration => f.write_str("incomplete platform context configuration"),
            Self::Layout(error) => error.fmt(f),
            Self::AddressSpace => f.write_str("invalid platform register placement"),
            Self::Overlap => f.write_str("platform register apertures overlap"),
        }
    }
}
impl std::error::Error for PlatformConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Layout(error) => Some(error),
            Self::FunctionCount
            | Self::ContextConfiguration
            | Self::AddressSpace
            | Self::Overlap => None,
        }
    }
}

/// Invalid managed hardware assembly.
#[derive(Debug)]
pub enum BuildError {
    /// The integration platform's configuration is invalid.
    Platform(PlatformConfigError),
    /// DMA construction failed.
    Dma(pdma_model_dma::BuildError),
    /// Hardware access-window construction failed.
    Window(pdma_model_hw_components::windows::WindowConfigError),
    /// Context-table construction failed.
    Context(pdma_model_hw_components::context::ContextConfigError),
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "managed assembly: {self:?}")
    }
}
impl std::error::Error for BuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Platform(error) => Some(error),
            Self::Dma(error) => Some(error),
            Self::Window(error) => Some(error),
            Self::Context(error) => Some(error),
        }
    }
}

/// Failure of a supervised hardware or firmware actor.
#[derive(Debug)]
pub enum RunError<E = hardware_interface_io::memory::MemoryAccessError> {
    /// Firmware boot or control service failed.
    Firmware(
        pdma_model_firmware::Error<
            hardware_interface_software::EnvAccessError<super::AccessError>,
            E,
        >,
    ),
    /// Firmware interrupt delivery failed.
    FirmwareInterrupt(
        hardware_interface_software::InterruptDispatchError<
            (),
            tokio::sync::watch::error::RecvError,
            Infallible,
        >,
    ),
    /// The hardware pipeline failed.
    Hardware(AsyncRunError<Infallible, pdma_model_dma::Error>),
    /// A persistent hardware operation unexpectedly completed.
    HardwareStopped,
}

impl<E: fmt::Debug> fmt::Display for RunError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "managed execution: {self:?}")
    }
}
impl<E: std::error::Error + 'static> std::error::Error for RunError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Firmware(error) => Some(error),
            Self::FirmwareInterrupt(error) => Some(error),
            Self::Hardware(AsyncRunError::Stage(error)) => Some(error),
            Self::Hardware(AsyncRunError::BackendInteraction(error)) => Some(error),
            Self::Hardware(AsyncRunError::Run(error)) => Some(error),
            Self::HardwareStopped => None,
        }
    }
}
