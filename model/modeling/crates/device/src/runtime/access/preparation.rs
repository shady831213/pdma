use crate::device::DeviceCore;
use crate::queue::exact::PdmaControlError;
use hardware_interface_runtime::{AccessOrigin, RegisterObservation};
use pdma_model_contract::abi::registers::{MmioAccessError, RegisterLocation};

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
    register: RegisterLocation,
    value: u64,
    pub(super) offset: u32,
    pub(super) request: Vec<u8>,
}

impl PreparedWrite {
    pub(crate) fn commit(&self, state: &mut DeviceCore) -> Result<bool, PdmaControlError> {
        state.write_mmio_register(self.register, self.value)
    }

    pub(crate) fn observation(&self, sequence: u64, origin: AccessOrigin) -> RegisterObservation {
        RegisterObservation::write(sequence, origin, self.offset, &self.request)
    }
}

pub(crate) fn prepare_read(
    state: &DeviceCore,
    register: RegisterLocation,
    offset: u32,
) -> Result<PreparedRead, MmioAccessError> {
    Ok(PreparedRead {
        response: state
            .read_register(register)
            .ok_or(MmioAccessError::ReservedOffset { offset })?,
        offset,
    })
}

pub(crate) fn prepare_write(
    register: RegisterLocation,
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

#[cfg(test)]
mod tests;
