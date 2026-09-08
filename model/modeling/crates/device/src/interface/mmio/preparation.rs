use crate::device::DeviceCore;
use crate::registers::narrow_u32;
use hardware_interface_runtime::{AccessOrigin, ObservationGateError, RegisterObservation};
use pdma_model_contract::abi::mmio::{MmioAccessError, RegisterId};
use std::fmt::{self, Display, Formatter};

/// Failure from an exact generated-view register operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MmioRegisterError {
    /// The generated PDMA register map rejected the transaction.
    Access(MmioAccessError),
    /// The core-owned ordered observer gate rejected committed emission.
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

pub(crate) struct PreparedRead {
    pub(crate) response: Vec<u8>,
    pub(super) offset: u32,
}

impl PreparedRead {
    pub(crate) fn observation(&self, sequence: u64) -> RegisterObservation {
        RegisterObservation::read(sequence, AccessOrigin::Direct, self.offset, &self.response)
    }
}

#[derive(Clone)]
pub(crate) struct PreparedWrite {
    register: RegisterId,
    value: u64,
    pub(super) offset: u32,
    pub(super) request: Vec<u8>,
}

impl PreparedWrite {
    pub(crate) fn commit(&self, state: &mut DeviceCore) -> bool {
        match self.register {
            RegisterId::GCommand => super::global::execute(state, narrow_u32(self.value)),
            register => {
                state.registers.write(register, &self.request);
                match register {
                    RegisterId::QCommand => super::queue::execute(state, self.value),
                    RegisterId::ICommand => super::irq::execute(state, narrow_u32(self.value)),
                    _ => false,
                }
            }
        }
    }

    pub(crate) fn observation(&self, sequence: u64, origin: AccessOrigin) -> RegisterObservation {
        RegisterObservation::write(sequence, origin, self.offset, &self.request)
    }
}

pub(crate) fn prepare_read(state: &DeviceCore, register: RegisterId, offset: u32) -> PreparedRead {
    PreparedRead {
        response: state.registers.read(register),
        offset,
    }
}

pub(crate) fn prepare_write(
    register: RegisterId,
    value: u64,
    offset: u32,
    request: &[u8],
) -> PreparedWrite {
    PreparedWrite {
        register,
        value,
        offset,
        request: request.to_vec(),
    }
}
