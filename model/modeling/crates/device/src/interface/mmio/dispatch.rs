use super::{global, irq, queue};
use crate::device::DeviceCore;
use crate::register_file::{MmioScalar, narrow_u32};
use pdma_model_contract::abi::mmio::RegisterId;

pub(crate) fn read(state: &DeviceCore, register: RegisterId) -> MmioScalar {
    MmioScalar::new(state.registers.read(register))
}

pub(crate) fn write(
    state: &mut DeviceCore,
    register: RegisterId,
    value: u64,
    request: &[u8],
) -> bool {
    state.registers.write(register, request);
    match register {
        RegisterId::GCommand => global::execute(state, narrow_u32(value)),
        RegisterId::QCommand => queue::execute(state, value),
        RegisterId::ICommand => irq::execute(state, narrow_u32(value)),
        _ => false,
    }
}
