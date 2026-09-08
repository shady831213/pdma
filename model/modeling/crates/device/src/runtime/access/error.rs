use crate::queue::exact::PdmaControlError;
use hardware_interface_runtime::ObservationGateError;
use pdma_model_contract::abi::registers::MmioAccessError;
use pdma_model_contract::semantic::DeviceEpoch;
use std::fmt::{self, Display, Formatter};

/// Failure from one device register access or its runtime observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MmioRegisterError {
    /// The canonical PDMA register map rejected the transaction.
    Access(MmioAccessError),
    /// Hardware rejected the requested register transition before commit.
    Control(PdmaControlError),
    /// The runtime-owned ordered observer gate rejected the access or emission.
    Observation(ObservationGateError),
    /// A register write committed, but its ordered observation could not be emitted.
    CommittedObservation(ObservationGateError),
    /// The endpoint binding expired before this access could mutate hardware.
    StaleEpoch {
        /// Current device epoch.
        expected: DeviceEpoch,
        /// Endpoint's bound epoch.
        actual: DeviceEpoch,
    },
}

impl Display for MmioRegisterError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Access(error) => Display::fmt(error, formatter),
            Self::Control(error) => Display::fmt(error, formatter),
            Self::Observation(error) | Self::CommittedObservation(error) => {
                Display::fmt(error, formatter)
            }
            Self::StaleEpoch { expected, actual } => write!(
                formatter,
                "stale register endpoint: expected {expected:?}, actual {actual:?}"
            ),
        }
    }
}

impl std::error::Error for MmioRegisterError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Access(error) => Some(error),
            Self::Control(error) => Some(error),
            Self::Observation(error) | Self::CommittedObservation(error) => Some(error),
            Self::StaleEpoch { .. } => None,
        }
    }
}

impl From<PdmaControlError> for MmioRegisterError {
    fn from(error: PdmaControlError) -> Self {
        Self::Control(error)
    }
}

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
