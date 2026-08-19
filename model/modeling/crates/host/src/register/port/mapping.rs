use crate::ModelError;
use crate::core::PortError;
use crate::register::{
    PdmaBatchError, PdmaProvisioningCause, PdmaProvisioningError, PdmaRegisterPortError,
};
use pdma_model_contract::abi::mmio::MmioResult;

pub(super) const fn provisioning(error: PdmaProvisioningError) -> ModelError {
    match error {
        PdmaProvisioningError::Provisioning(
            PdmaProvisioningCause::AddressExhausted | PdmaProvisioningCause::StorageExhausted,
        )
        | PdmaProvisioningError::LengthOutOfRange => ModelError::DmaAllocationExhausted,
        _ => ModelError::DmaSubmissionState,
    }
}

pub(super) const fn batch(error: PdmaBatchError) -> ModelError {
    match error {
        PdmaBatchError::Provisioning(error) => provisioning(error),
        PdmaBatchError::Result(details)
            if matches!(
                details.result,
                MmioResult::DmaAddressExhausted | MmioResult::DmaStorageExhausted
            ) =>
        {
            ModelError::DmaAllocationExhausted
        }
        _ => ModelError::DmaSubmissionState,
    }
}

pub(super) const fn legacy(error: &PdmaRegisterPortError) -> ModelError {
    match error {
        PdmaRegisterPortError::Cpu(source) => match source {
            hardware_interface_runtime::CpuContextError::Inactive => {
                ModelError::SoftwareCpuInactive
            }
            hardware_interface_runtime::CpuContextError::Mismatch => {
                ModelError::SoftwareCpuMismatch
            }
            hardware_interface_runtime::CpuContextError::StateBusy => {
                ModelError::SoftwareCpuStateBusy
            }
        },
        PdmaRegisterPortError::Semantic(source) => semantic(*source),
        PdmaRegisterPortError::Provisioning(source) => provisioning(*source),
        PdmaRegisterPortError::Batch(source) => batch(*source),
        PdmaRegisterPortError::Register(_)
        | PdmaRegisterPortError::Claim(_)
        | PdmaRegisterPortError::Memory(_)
        | PdmaRegisterPortError::Response(_) => ModelError::DmaSubmissionState,
    }
}

const fn semantic(error: PortError) -> ModelError {
    match error {
        PortError::StaleEpoch { expected, actual } => {
            ModelError::StaleDeviceEpoch { expected, actual }
        }
        PortError::ZeroCapacity => ModelError::ZeroCapacity,
        PortError::RingFull => ModelError::RingFull,
        PortError::UnknownBuffer(id) => ModelError::UnknownBuffer(id),
        PortError::UnknownQueue(id) => ModelError::UnknownQueue(id),
        PortError::UnknownCompletionRing(id) => ModelError::UnknownCompletionRing(id),
        PortError::UnknownIrqSource(source) => ModelError::UnknownIrqSource(source),
        PortError::IrqSourceActive(source) => ModelError::IrqSourceActive(source),
        PortError::RangeOverflow => ModelError::RangeOverflow,
        PortError::OutOfBounds { buffer, len, end } => ModelError::OutOfBounds { buffer, len, end },
        PortError::AllocationExhausted => ModelError::DmaAllocationExhausted,
        PortError::SubmissionState => ModelError::DmaSubmissionState,
        PortError::StepBudgetExhausted { max_steps } => {
            ModelError::StepBudgetExhausted { max_steps }
        }
    }
}
