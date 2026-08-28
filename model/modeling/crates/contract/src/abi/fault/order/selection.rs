use core::cmp::Ordering;
use core::fmt;

use super::types::{
    AbiFaultOrder, AbiFaultSelectionError, AbiOperandFaultStep, AbiOperandVisit, OrderKey,
};
use super::{operand_context, operation_for, order_key};
use crate::abi::descriptor::{
    AbiCommandHeader, DescriptorViolationKind, is_indirect_alignment_location,
    is_indirect_count_location, is_reserved_location, is_scalar_location, location_offset,
};
use crate::abi::fault::{AbiEventKind, AbiFault, FaultTuple};

/// One canonical fault paired with its typed first-match location.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbiFaultCandidate {
    fault: AbiFault,
    order: AbiFaultOrder,
}

impl AbiFaultCandidate {
    /// Validates that a canonical fault matches the supplied typed location.
    pub fn try_new(fault: AbiFault, order: AbiFaultOrder) -> Result<Self, AbiFaultSelectionError> {
        if !fault.is_canonical() {
            return Err(AbiFaultSelectionError::InvalidFaultTuple);
        }
        if !fault_matches_order(fault, order) {
            return Err(AbiFaultSelectionError::FaultDoesNotMatchOrder);
        }
        Ok(Self { fault, order })
    }
}

/// Selects the unique first fault from an unsorted candidate iterator.
pub fn select_canonical_fault(
    candidates: impl IntoIterator<Item = AbiFaultCandidate>,
) -> Result<Option<AbiFault>, AbiFaultSelectionError> {
    let mut operation = None;
    let mut selected: Option<(OrderKey, AbiFault)> = None;
    for candidate in candidates {
        if let Some(candidate_operation) = operation_for(candidate.order) {
            match operation {
                Some(current) if current != candidate_operation => {
                    return Err(AbiFaultSelectionError::MixedOperations);
                }
                Some(_) => {}
                None => operation = Some(candidate_operation),
            }
        }
        let key = order_key(candidate.order);
        match selected {
            Some((current, _)) => match key.cmp(&current) {
                Ordering::Less => selected = Some((key, candidate.fault)),
                Ordering::Equal => {
                    return Err(AbiFaultSelectionError::AmbiguousFirstFault);
                }
                Ordering::Greater => {}
            },
            None => selected = Some((key, candidate.fault)),
        }
    }
    Ok(selected.map(|(_, fault)| fault))
}

impl fmt::Display for AbiFaultSelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFaultTuple => formatter.write_str("fault tuple is not canonical"),
            Self::FaultDoesNotMatchOrder => {
                formatter.write_str("fault does not match its typed order location")
            }
            Self::MixedOperations => formatter.write_str("candidate set mixes operations"),
            Self::AmbiguousFirstFault => {
                formatter.write_str("candidate set has two faults at the first location")
            }
        }
    }
}

impl std::error::Error for AbiFaultSelectionError {}

fn fault_matches_order(fault: AbiFault, order: AbiFaultOrder) -> bool {
    let tuple = fault.parts();
    match order {
        AbiFaultOrder::Descriptor(kind) => descriptor_matches(tuple, kind),
        AbiFaultOrder::Operand { visit, step } => {
            operand_matches(tuple, visit) && step_matches(tuple.kind, visit, step)
        }
    }
}

fn descriptor_matches(tuple: FaultTuple, kind: DescriptorViolationKind) -> bool {
    match kind {
        DescriptorViolationKind::Header(location) => header_matches(tuple, location),
        DescriptorViolationKind::Reserved { opcode, location } => {
            is_reserved_location(opcode, location)
                && tuple.kind == AbiEventKind::InvalidReserved
                && tuple.index == location_offset(location)
        }
        DescriptorViolationKind::Scalar(location) => scalar_matches(tuple, location),
    }
}

fn header_matches(tuple: FaultTuple, location: hardware_interface_schema::SchemaLocation) -> bool {
    let kind = if location
        == AbiCommandHeader::VERSION_IDENTITY.at_byte(AbiCommandHeader::VERSION_OFFSET)
    {
        AbiEventKind::InvalidVersion
    } else if location == AbiCommandHeader::SIZE_IDENTITY.at_byte(AbiCommandHeader::SIZE_OFFSET) {
        AbiEventKind::InvalidSize
    } else if location == AbiCommandHeader::OPCODE_IDENTITY.at_byte(AbiCommandHeader::OPCODE_OFFSET)
    {
        AbiEventKind::InvalidOpcode
    } else if location == AbiCommandHeader::FLAGS_IDENTITY.at_byte(AbiCommandHeader::FLAGS_OFFSET) {
        AbiEventKind::InvalidFlags
    } else {
        return false;
    };
    tuple.kind == kind && tuple.index == location_offset(location)
}

fn scalar_matches(tuple: FaultTuple, location: hardware_interface_schema::SchemaLocation) -> bool {
    if !is_scalar_location(location) {
        return false;
    }
    let (kind, index) = if is_indirect_count_location(location) {
        (AbiEventKind::InvalidIndirectCount, 0)
    } else if is_indirect_alignment_location(location) {
        (AbiEventKind::InvalidIndirectAlignment, 0)
    } else {
        (AbiEventKind::ValueOutOfRange, location_offset(location))
    };
    tuple.kind == kind && tuple.index == index
}

const fn operand_matches(tuple: FaultTuple, visit: AbiOperandVisit) -> bool {
    let (stage, access, index) = operand_context(visit);
    tuple.stage == stage.value() && tuple.access == access.value() && tuple.index == index
}

fn step_matches(kind: AbiEventKind, visit: AbiOperandVisit, step: AbiOperandFaultStep) -> bool {
    match step {
        AbiOperandFaultStep::CheckedStart => {
            kind == AbiEventKind::RangeOverflow
                && matches!(
                    visit,
                    AbiOperandVisit::StridedSource { .. }
                        | AbiOperandVisit::StridedDestination { .. }
                )
        }
        AbiOperandFaultStep::CheckedEnd => kind == AbiEventKind::RangeOverflow,
        AbiOperandFaultStep::Unmapped => kind == AbiEventKind::UnmappedDmaAddress,
        AbiOperandFaultStep::OutOfBounds => kind == AbiEventKind::OutOfBounds,
    }
}
