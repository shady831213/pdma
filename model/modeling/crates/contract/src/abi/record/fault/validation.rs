use super::AbiFault;
use crate::abi::record::{AbiEventAccess, AbiEventKind, AbiEventStage};
use crate::abi::{
    DESCRIPTOR_SIZE, DescriptorHeaderSchema, DescriptorSchema, INDIRECT_ENTRY_ALIGNMENT,
    MAX_INDIRECT_DESTINATIONS, MAX_TRANSFER_BYTES, VECTOR_ADD_OPCODE,
};

pub(super) fn is_canonical(fault: AbiFault) -> bool {
    let tuple = fault.tuple();
    let operand = canonical_operand(tuple.4, tuple.5);
    match fault.kind() {
        AbiEventKind::UnmappedIova => operand && tuple.3 == 0,
        AbiEventKind::RangeOverflow => operand && range_overflow(tuple),
        AbiEventKind::OutOfBounds => operand && tuple.3 > tuple.1,
        AbiEventKind::InvalidVersion => {
            descriptor(tuple, 1, DescriptorHeaderSchema::VERSION_OFFSET as u64)
        }
        AbiEventKind::InvalidSize => descriptor(
            tuple,
            u64::from(DESCRIPTOR_SIZE),
            DescriptorHeaderSchema::SIZE_OFFSET as u64,
        ),
        AbiEventKind::InvalidOpcode => {
            (tuple.2 == 0 || tuple.2 > u64::from(VECTOR_ADD_OPCODE))
                && descriptor(
                    tuple,
                    u64::from(VECTOR_ADD_OPCODE),
                    DescriptorHeaderSchema::OPCODE_OFFSET as u64,
                )
        }
        AbiEventKind::InvalidFlags => {
            descriptor(tuple, 0, DescriptorHeaderSchema::FLAGS_OFFSET as u64)
        }
        AbiEventKind::InvalidReserved => {
            tuple.2 != 0
                && tuple.3 == 0
                && matches!(
                    tuple.6,
                    value if value == DescriptorSchema::WORD3_OFFSET as u64
                        || value == DescriptorSchema::WORD4_OFFSET as u64
                        || value == DescriptorSchema::WORD5_OFFSET as u64
                )
                && descriptor_context(tuple)
        }
        AbiEventKind::InvalidIndirectCount => {
            (tuple.2 == 0 || tuple.2 > u64::from(MAX_INDIRECT_DESTINATIONS))
                && tuple.3 == u64::from(MAX_INDIRECT_DESTINATIONS)
                && tuple.6 == 0
                && indirect_context(tuple)
        }
        AbiEventKind::InvalidIndirectAlignment => {
            tuple.1 % INDIRECT_ENTRY_ALIGNMENT != 0
                && tuple.2 == tuple.1 % INDIRECT_ENTRY_ALIGNMENT
                && tuple.3 == INDIRECT_ENTRY_ALIGNMENT
                && tuple.6 == 0
                && indirect_context(tuple)
        }
        AbiEventKind::ValueOutOfRange => {
            tuple.2 > tuple.3
                && tuple.3 == u64::from(MAX_TRANSFER_BYTES)
                && matches!(
                    tuple.6,
                    value if value == DescriptorSchema::WORD2_OFFSET as u64
                        || value == DescriptorSchema::WORD3_OFFSET as u64
                        || value == DescriptorSchema::WORD4_OFFSET as u64
                        || value == DescriptorSchema::WORD5_OFFSET as u64
                )
                && descriptor_context(tuple)
        }
    }
}

const fn descriptor(
    tuple: (AbiEventKind, u64, u64, u64, u16, u16, u64),
    expected: u64,
    index: u64,
) -> bool {
    tuple.2 != expected && tuple.3 == expected && tuple.6 == index && descriptor_context(tuple)
}

const fn descriptor_context(tuple: (AbiEventKind, u64, u64, u64, u16, u16, u64)) -> bool {
    tuple.4 == AbiEventStage::Descriptor.value() && tuple.5 == AbiEventAccess::None.value()
}

const fn indirect_context(tuple: (AbiEventKind, u64, u64, u64, u16, u16, u64)) -> bool {
    tuple.4 == AbiEventStage::IndirectList.value() && tuple.5 == AbiEventAccess::None.value()
}

const fn canonical_operand(stage: u16, access: u16) -> bool {
    stage == AbiEventStage::IndirectList.value() && access == AbiEventAccess::Read.value()
        || stage == AbiEventStage::Source.value() && access == AbiEventAccess::Read.value()
        || stage == AbiEventStage::Destination.value() && access == AbiEventAccess::Write.value()
}

fn range_overflow(tuple: (AbiEventKind, u64, u64, u64, u16, u16, u64)) -> bool {
    if tuple.4 == AbiEventStage::IndirectList.value() {
        return tuple.3 == INDIRECT_ENTRY_ALIGNMENT
            && tuple.6 == 0
            && tuple
                .2
                .checked_mul(INDIRECT_ENTRY_ALIGNMENT)
                .and_then(|extent| tuple.1.checked_add(extent))
                .is_none();
    }
    if tuple.3 == 0 {
        tuple.1.checked_add(tuple.2).is_none()
    } else {
        tuple.2 == tuple.6
            && tuple
                .2
                .checked_mul(tuple.3)
                .and_then(|offset| tuple.1.checked_add(offset))
                .is_none()
    }
}
