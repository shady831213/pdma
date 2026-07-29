use core::cmp::Ordering;
use core::fmt;

use super::types::{
    AbiFaultOrder, AbiFaultSelectionError, AbiHeaderField, AbiOperandFaultStep, AbiOperandVisit,
    AbiScalarField, OrderKey,
};
use super::{operand_context, operation_for, order_key, reserved_offset};
use crate::abi::record::{AbiEventKind, AbiFault};

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
    let tuple = fault.tuple();
    match order {
        AbiFaultOrder::Header(field) => header_matches(tuple, field),
        AbiFaultOrder::Reserved(field) => {
            tuple.0 == AbiEventKind::InvalidReserved && tuple.6 == reserved_offset(field)
        }
        AbiFaultOrder::Scalar(field) => scalar_matches(tuple, field),
        AbiFaultOrder::Operand { visit, step } => {
            operand_matches(tuple, visit) && step_matches(tuple.0, visit, step)
        }
    }
}

fn header_matches(tuple: FaultTuple, field: AbiHeaderField) -> bool {
    let (kind, index) = match field {
        AbiHeaderField::Version => (AbiEventKind::InvalidVersion, 0),
        AbiHeaderField::Size => (AbiEventKind::InvalidSize, 6),
        AbiHeaderField::Opcode => (AbiEventKind::InvalidOpcode, 2),
        AbiHeaderField::Flags => (AbiEventKind::InvalidFlags, 4),
    };
    tuple.0 == kind && tuple.6 == index
}

fn scalar_matches(tuple: FaultTuple, field: AbiScalarField) -> bool {
    let (kind, index) = match field {
        AbiScalarField::CopyLength
        | AbiScalarField::StridedRows
        | AbiScalarField::BroadcastLength => (AbiEventKind::ValueOutOfRange, 32),
        AbiScalarField::StridedRowBytes | AbiScalarField::VectorLength => {
            (AbiEventKind::ValueOutOfRange, 40)
        }
        AbiScalarField::StridedSourceStride => (AbiEventKind::ValueOutOfRange, 48),
        AbiScalarField::StridedDestinationStride => (AbiEventKind::ValueOutOfRange, 56),
        AbiScalarField::BroadcastDestinationCount => (AbiEventKind::InvalidIndirectCount, 0),
        AbiScalarField::BroadcastListAlignment => (AbiEventKind::InvalidIndirectAlignment, 0),
    };
    tuple.0 == kind && tuple.6 == index
}

const fn operand_matches(tuple: FaultTuple, visit: AbiOperandVisit) -> bool {
    let (stage, access, index) = operand_context(visit);
    tuple.4 == stage.value() && tuple.5 == access.value() && tuple.6 == index
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
        AbiOperandFaultStep::Unmapped => kind == AbiEventKind::UnmappedIova,
        AbiOperandFaultStep::OutOfBounds => kind == AbiEventKind::OutOfBounds,
    }
}

type FaultTuple = (AbiEventKind, u64, u64, u64, u16, u16, u64);
