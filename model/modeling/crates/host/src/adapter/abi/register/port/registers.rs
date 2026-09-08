use super::{CanonicalAccess, PdmaPortResult, PdmaRegisterPort};
use hardware_interface_runtime::RegisterAccess;
use pdma_model_contract::abi::mmio::{PdmaMmioMap, ResponseCodecError, register_at};

pub(super) const fn queue_offset(local: usize) -> usize {
    PdmaMmioMap::QUEUE.offset() + local
}

pub(super) const fn irq_offset(local: usize) -> usize {
    PdmaMmioMap::IRQ.offset() + local
}

pub(super) async fn write<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    address: usize,
    value: u64,
) -> PdmaPortResult<()> {
    let register = register(address)?;
    let mut bytes = vec![0; register.width()];
    register
        .definition()
        .view_mut(&mut bytes)
        .map_err(ResponseCodecError::from)?
        .set_value(value)
        .map_err(ResponseCodecError::from)?;
    port.poll_register(RegisterAccess::write(
        &port.access,
        offset(register.offset())?,
        &bytes,
    ))
    .await
}

pub(super) async fn read<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    address: usize,
) -> PdmaPortResult<u64> {
    let register = register(address)?;
    let mut bytes = vec![0; register.width()];
    port.poll_register(RegisterAccess::read(
        &port.access,
        offset(register.offset())?,
        &mut bytes,
    ))
    .await?;
    register
        .definition()
        .view(&bytes)
        .map_err(ResponseCodecError::from)?
        .value()
        .map_err(ResponseCodecError::from)
        .map_err(Into::into)
}

pub(super) async fn read_bank<A: CanonicalAccess>(
    port: &PdmaRegisterPort<A>,
    offsets: impl Iterator<Item = usize> + Send,
) -> PdmaPortResult<Vec<u8>> {
    let registers = offsets.map(register).collect::<PdmaPortResult<Vec<_>>>()?;
    let capacity = registers.iter().map(|register| register.width()).sum();
    let mut bank = Vec::with_capacity(capacity);
    for register in registers {
        let mut bytes = vec![0; register.width()];
        port.poll_register(RegisterAccess::read(
            &port.access,
            offset(register.offset())?,
            &mut bytes,
        ))
        .await?;
        bank.extend_from_slice(&bytes);
    }
    Ok(bank)
}

fn register(address: usize) -> PdmaPortResult<pdma_model_contract::abi::mmio::RegisterSpec> {
    register_at(address)
        .ok_or(crate::PortError::SubmissionState)
        .map_err(Into::into)
}

fn offset(value: usize) -> PdmaPortResult<u32> {
    u32::try_from(value)
        .map_err(|_| crate::PortError::SubmissionState)
        .map_err(Into::into)
}
