use super::{CanonicalAccess, PdmaPortResult, PdmaRegisterPort};
use crate::PortError;
use hardware_interface_runtime::RegisterAccess;
use pdma_model_contract::abi::registers::{GlobalRegisters, RegisterLocation, locate};
use pdma_model_contract::semantic::{DeviceEpoch, QueueId};

pub(super) fn queue_offset(queue: QueueId, local: usize) -> PdmaPortResult<u32> {
    let index = u32::try_from(queue.raw()).map_err(|_| PortError::SubmissionState)?;
    pdma_model_contract::abi::registers::queue_offset(index, local)
        .map_err(|_| PortError::SubmissionState.into())
}

pub(super) async fn write<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    address: u32,
    value: u64,
) -> PdmaPortResult<()> {
    let bytes = value.to_le_bytes();
    let request = bytes
        .get(..width(address)?)
        .ok_or(PortError::SubmissionState)?;
    port.poll_register(RegisterAccess::write(&port.access, address, request))
        .await
}

pub(super) async fn read<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    address: u32,
) -> PdmaPortResult<u64> {
    let mut bytes = [0; 8];
    let response = bytes
        .get_mut(..width(address)?)
        .ok_or(PortError::SubmissionState)?;
    port.poll_register(RegisterAccess::read(&port.access, address, response))
        .await?;
    let value = u64::from_le_bytes(bytes);
    let epoch_offset =
        u32::try_from(GlobalRegisters::EPOCH.offset()).map_err(|_| PortError::SubmissionState)?;
    let observed = if address == epoch_offset {
        DeviceEpoch::new(value)
    } else {
        let mut epoch_bytes = [0; 8];
        port.poll_register(RegisterAccess::read(
            &port.access,
            epoch_offset,
            &mut epoch_bytes,
        ))
        .await?;
        DeviceEpoch::new(u64::from_le_bytes(epoch_bytes))
    };
    let bound = port.with_state(|state| state.epoch)?;
    if observed != bound {
        return Err(PortError::StaleEpoch {
            expected: observed,
            actual: bound,
        }
        .into());
    }
    Ok(value)
}

pub(super) const fn committed(result: &PdmaPortResult<()>) -> bool {
    match result {
        Ok(()) => true,
        Err(super::PdmaRegisterPortError::Register(error)) => error.is_committed(),
        Err(_) => false,
    }
}

fn width(address: u32) -> PdmaPortResult<usize> {
    match locate(address).ok_or(PortError::SubmissionState)? {
        RegisterLocation::Global { register } => Ok(register.width()),
        RegisterLocation::Queue { register, .. } => Ok(register.width()),
        RegisterLocation::Irq { register, .. } => Ok(register.width()),
    }
}
