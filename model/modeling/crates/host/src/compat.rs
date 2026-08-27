//! Explicit host compatibility ports for the supported device access surfaces.

mod async_device;
mod semantic_device;

pub use async_device::AsyncDevicePort;
pub use semantic_device::SemanticDevicePort;

use crate::PortError;
use pdma_model_device::DeviceError;

const fn port_error(source: DeviceError) -> PortError {
    match source {
        DeviceError::StaleDeviceEpoch { expected, actual } => {
            PortError::StaleEpoch { expected, actual }
        }
        DeviceError::ZeroCapacity => PortError::ZeroCapacity,
        DeviceError::RingFull => PortError::RingFull,
        DeviceError::UnknownBuffer(id) => PortError::UnknownBuffer(id),
        DeviceError::UnknownQueue(id) => PortError::UnknownQueue(id),
        DeviceError::UnknownCompletionRing(id) => PortError::UnknownCompletionRing(id),
        DeviceError::UnknownIrqSource(source) => PortError::UnknownIrqSource(source),
        DeviceError::IrqSourceActive(source) => PortError::IrqSourceActive(source),
        DeviceError::RangeOverflow => PortError::RangeOverflow,
        DeviceError::OutOfBounds { buffer, len, end } => {
            PortError::OutOfBounds { buffer, len, end }
        }
        DeviceError::DmaAllocationExhausted => PortError::AllocationExhausted,
        DeviceError::DmaSubmissionState => PortError::SubmissionState,
        DeviceError::StepBudgetExhausted { max_steps } => {
            PortError::StepBudgetExhausted { max_steps }
        }
    }
}
