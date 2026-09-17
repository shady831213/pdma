use hardware_interface_pipeline::AsyncRunError;
use std::{convert::Infallible, fmt};

/// Invalid managed hardware assembly.
#[derive(Debug)]
pub enum BuildError {
    /// DMA construction failed.
    Dma(pdma_model_dma::BuildError),
    /// DMA register layout failed.
    Layout(pdma_model_contract::DeviceLayoutError),
    /// Work and management windows overlap.
    Window(pdma_model_hw_components::WindowConfigError),
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "managed assembly: {self:?}")
    }
}
impl std::error::Error for BuildError {}

/// Failure of a supervised hardware or firmware actor.
#[derive(Debug)]
pub enum RunError {
    /// Firmware boot or control service failed.
    Firmware(pdma_model_firmware::Error<super::AccessError>),
    /// The hardware pipeline failed.
    Hardware(AsyncRunError<Infallible, pdma_model_dma::Error>),
    /// A persistent hardware operation unexpectedly completed.
    HardwareStopped,
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "managed execution: {self:?}")
    }
}
impl std::error::Error for RunError {}
