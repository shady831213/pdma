use core::fmt;

use crate::abi::{
    ABI_VERSION, DESCRIPTOR_FLAGS_MASK, DESCRIPTOR_SIZE, INDIRECT_ENTRY_ALIGNMENT,
    MAX_INDIRECT_DESTINATIONS, MAX_TRANSFER_BYTES, VECTOR_ADD_OPCODE,
};

use super::super::location_offset;
use super::{DescriptorViolation, DescriptorViolationValue};

impl fmt::Display for DescriptorViolation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            DescriptorViolationValue::InvalidVersion { observed } => write!(
                formatter,
                "descriptor version {observed} is not {ABI_VERSION}"
            ),
            DescriptorViolationValue::InvalidSize { observed } => write!(
                formatter,
                "descriptor size {observed} is not {DESCRIPTOR_SIZE}"
            ),
            DescriptorViolationValue::InvalidOpcode { observed } => write!(
                formatter,
                "descriptor opcode {observed} is outside 1..={VECTOR_ADD_OPCODE}"
            ),
            DescriptorViolationValue::InvalidFlags { observed } => write!(
                formatter,
                "descriptor flags {observed:#x} exceed mask {DESCRIPTOR_FLAGS_MASK:#x}"
            ),
            DescriptorViolationValue::Reserved {
                location, observed, ..
            } => write!(
                formatter,
                "descriptor reserved word at {} is {observed:#x}",
                location_offset(location)
            ),
            DescriptorViolationValue::Scalar { location, observed } => write!(
                formatter,
                "descriptor value {observed} at {} exceeds {MAX_TRANSFER_BYTES}",
                location_offset(location)
            ),
            DescriptorViolationValue::IndirectCount { observed, .. } => write!(
                formatter,
                "indirect destination count {observed} is outside 1..={MAX_INDIRECT_DESTINATIONS}"
            ),
            DescriptorViolationValue::IndirectAlignment { list } => write!(
                formatter,
                "indirect list address {:#x} is not aligned to {INDIRECT_ENTRY_ALIGNMENT}",
                list.raw()
            ),
        }
    }
}
