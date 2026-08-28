use super::{AbiFault, FaultTuple};
use crate::abi::descriptor::{AbiCommand, AbiCommandHeader, location_offset};
use crate::abi::fault::{AbiEventAccess, AbiEventKind, AbiEventStage};
use crate::abi::{
    DESCRIPTOR_SIZE, INDIRECT_ENTRY_ALIGNMENT, MAX_INDIRECT_DESTINATIONS, MAX_TRANSFER_BYTES,
    VECTOR_ADD_OPCODE,
};

pub(super) fn is_canonical(fault: AbiFault) -> bool {
    let tuple = fault.parts();
    let operand = canonical_operand(tuple.stage, tuple.access);
    match fault.kind() {
        AbiEventKind::UnmappedDmaAddress => operand && tuple.expected == 0,
        AbiEventKind::RangeOverflow => operand && range_overflow(tuple),
        AbiEventKind::OutOfBounds => operand && tuple.expected > tuple.locator,
        AbiEventKind::InvalidVersion => descriptor(
            tuple,
            1,
            location_offset(
                AbiCommandHeader::VERSION_IDENTITY.at_byte(AbiCommandHeader::VERSION_OFFSET),
            ),
        ),
        AbiEventKind::InvalidSize => descriptor(
            tuple,
            u64::from(DESCRIPTOR_SIZE),
            location_offset(AbiCommandHeader::SIZE_IDENTITY.at_byte(AbiCommandHeader::SIZE_OFFSET)),
        ),
        AbiEventKind::InvalidOpcode => {
            (tuple.observed == 0 || tuple.observed > u64::from(VECTOR_ADD_OPCODE))
                && descriptor(
                    tuple,
                    u64::from(VECTOR_ADD_OPCODE),
                    location_offset(
                        AbiCommandHeader::OPCODE_IDENTITY.at_byte(AbiCommandHeader::OPCODE_OFFSET),
                    ),
                )
        }
        AbiEventKind::InvalidFlags => descriptor(
            tuple,
            0,
            location_offset(
                AbiCommandHeader::FLAGS_IDENTITY.at_byte(AbiCommandHeader::FLAGS_OFFSET),
            ),
        ),
        AbiEventKind::InvalidReserved => {
            tuple.observed != 0
                && tuple.expected == 0
                && matches_descriptor_word(tuple.index, 3)
                && descriptor_context(tuple)
        }
        AbiEventKind::InvalidIndirectCount => {
            (tuple.observed == 0 || tuple.observed > u64::from(MAX_INDIRECT_DESTINATIONS))
                && tuple.expected == u64::from(MAX_INDIRECT_DESTINATIONS)
                && tuple.index == 0
                && indirect_context(tuple)
        }
        AbiEventKind::InvalidIndirectAlignment => {
            tuple.locator % INDIRECT_ENTRY_ALIGNMENT != 0
                && tuple.observed == tuple.locator % INDIRECT_ENTRY_ALIGNMENT
                && tuple.expected == INDIRECT_ENTRY_ALIGNMENT
                && tuple.index == 0
                && indirect_context(tuple)
        }
        AbiEventKind::ValueOutOfRange => {
            tuple.observed > tuple.expected
                && tuple.expected == u64::from(MAX_TRANSFER_BYTES)
                && matches_descriptor_word(tuple.index, 2)
                && descriptor_context(tuple)
        }
    }
}

fn matches_descriptor_word(index: u64, first: usize) -> bool {
    (first..=5).any(|word| {
        let offset = match word {
            2 => AbiCommand::WORD2_OFFSET,
            3 => AbiCommand::WORD3_OFFSET,
            4 => AbiCommand::WORD4_OFFSET,
            5 => AbiCommand::WORD5_OFFSET,
            _ => return false,
        };
        index == location_offset(AbiCommand::PAYLOAD_IDENTITY.at_byte(offset))
    })
}

const fn descriptor(tuple: FaultTuple, expected: u64, index: u64) -> bool {
    tuple.observed != expected
        && tuple.expected == expected
        && tuple.index == index
        && descriptor_context(tuple)
}

const fn descriptor_context(tuple: FaultTuple) -> bool {
    tuple.stage == AbiEventStage::Descriptor.value() && tuple.access == AbiEventAccess::None.value()
}

const fn indirect_context(tuple: FaultTuple) -> bool {
    tuple.stage == AbiEventStage::IndirectList.value()
        && tuple.access == AbiEventAccess::None.value()
}

const fn canonical_operand(stage: u16, access: u16) -> bool {
    stage == AbiEventStage::IndirectList.value() && access == AbiEventAccess::Read.value()
        || stage == AbiEventStage::Source.value() && access == AbiEventAccess::Read.value()
        || stage == AbiEventStage::Destination.value() && access == AbiEventAccess::Write.value()
}

fn range_overflow(tuple: FaultTuple) -> bool {
    if tuple.stage == AbiEventStage::IndirectList.value() {
        return tuple.expected == INDIRECT_ENTRY_ALIGNMENT
            && tuple.index == 0
            && tuple
                .observed
                .checked_mul(INDIRECT_ENTRY_ALIGNMENT)
                .and_then(|extent| tuple.locator.checked_add(extent))
                .is_none();
    }
    if tuple.expected == 0 {
        tuple.locator.checked_add(tuple.observed).is_none()
    } else {
        tuple.observed == tuple.index
            && tuple
                .observed
                .checked_mul(tuple.expected)
                .and_then(|offset| tuple.locator.checked_add(offset))
                .is_none()
    }
}
