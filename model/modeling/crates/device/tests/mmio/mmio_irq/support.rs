pub(super) use super::super::mmio_queue::support::*;
use pdma_model_contract::abi::registers::{IrqSourceRegisters, irq_offset};
use pdma_model_device::MmioRegisterEndpoint;

pub(in crate::mmio) fn i(offset: usize) -> TestResult<u32> {
    Ok(irq_offset(0, 0, offset)?)
}

pub(in crate::mmio) async fn enable(endpoint: &MmioRegisterEndpoint) -> TestResult {
    write_u32(endpoint, i(IrqSourceRegisters::ENABLED.offset())?, 1).await?;
    write_u32(endpoint, i(IrqSourceRegisters::MASKED.offset())?, 0).await?;
    Ok(())
}
