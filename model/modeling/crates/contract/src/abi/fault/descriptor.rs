use crate::abi::descriptor::{DescriptorViolation, DescriptorViolationValue, location_offset};
use crate::abi::{AbiFault, DmaAddress};

impl DescriptorViolation {
    /// Converts the validated violation into its canonical hardware fault.
    pub fn into_abi_fault(self, descriptor_slot: DmaAddress) -> AbiFault {
        match self.value() {
            DescriptorViolationValue::InvalidVersion { observed } => {
                AbiFault::invalid_version(descriptor_slot, observed)
            }
            DescriptorViolationValue::InvalidSize { observed } => {
                AbiFault::invalid_size(descriptor_slot, observed)
            }
            DescriptorViolationValue::InvalidOpcode { observed } => {
                AbiFault::invalid_opcode(descriptor_slot, observed)
            }
            DescriptorViolationValue::InvalidFlags { observed } => {
                AbiFault::invalid_flags(descriptor_slot, observed)
            }
            DescriptorViolationValue::Reserved {
                location, observed, ..
            } => AbiFault::invalid_reserved(descriptor_slot, location_offset(location), observed),
            DescriptorViolationValue::Scalar { location, observed } => {
                AbiFault::value_out_of_range(descriptor_slot, location_offset(location), observed)
            }
            DescriptorViolationValue::IndirectCount { list, observed } => {
                AbiFault::invalid_indirect_count(list, observed)
            }
            DescriptorViolationValue::IndirectAlignment { list } => {
                AbiFault::invalid_indirect_alignment(list)
            }
        }
    }
}
