use hardware_interface_runtime::ObservationGateError;
use pdma_model_contract::abi::mmio::MmioAccessError;
use std::fmt::{self, Display, Formatter};

/// Failure from one device register access or its runtime observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MmioRegisterError {
    /// The canonical PDMA register map rejected the transaction.
    Access(MmioAccessError),
    /// The runtime-owned ordered observer gate rejected the access or emission.
    Observation(ObservationGateError),
}

impl Display for MmioRegisterError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Access(error) => Display::fmt(error, formatter),
            Self::Observation(error) => Display::fmt(error, formatter),
        }
    }
}

impl std::error::Error for MmioRegisterError {}

impl From<MmioAccessError> for MmioRegisterError {
    fn from(error: MmioAccessError) -> Self {
        Self::Access(error)
    }
}

impl From<ObservationGateError> for MmioRegisterError {
    fn from(error: ObservationGateError) -> Self {
        Self::Observation(error)
    }
}
