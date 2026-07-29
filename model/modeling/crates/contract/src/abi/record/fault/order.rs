mod selection;
mod types;

pub use selection::{AbiFaultCandidate, select_canonical_fault};
pub use types::{
    AbiFaultOrder, AbiFaultSelectionError, AbiHeaderField, AbiOperandFaultStep, AbiOperandVisit,
    AbiReservedField, AbiScalarField,
};

use crate::abi::record::{AbiEventAccess, AbiEventStage};
use types::{Operation, OrderKey};

const fn operation_for(order: AbiFaultOrder) -> Option<Operation> {
    match order {
        AbiFaultOrder::Header(_) => None,
        AbiFaultOrder::Reserved(field) => Some(reserved_operation(field)),
        AbiFaultOrder::Scalar(field) => Some(scalar_operation(field)),
        AbiFaultOrder::Operand { visit, .. } => Some(operand_operation(visit)),
    }
}

const fn order_key(order: AbiFaultOrder) -> OrderKey {
    match order {
        AbiFaultOrder::Header(field) => OrderKey::new(0, header_rank(field), 0, 0, 0),
        AbiFaultOrder::Reserved(field) => OrderKey::new(1, reserved_offset(field), 0, 0, 0),
        AbiFaultOrder::Scalar(field) => OrderKey::new(2, scalar_rank(field), 0, 0, 0),
        AbiFaultOrder::Operand { visit, step } => {
            let (major, index, side) = operand_rank(visit);
            OrderKey::new(3, major, index, side, step_rank(step))
        }
    }
}

const fn header_rank(field: AbiHeaderField) -> u64 {
    match field {
        AbiHeaderField::Version => 0,
        AbiHeaderField::Size => 1,
        AbiHeaderField::Opcode => 2,
        AbiHeaderField::Flags => 3,
    }
}

const fn reserved_operation(field: AbiReservedField) -> Operation {
    match field {
        AbiReservedField::CopyOffset40
        | AbiReservedField::CopyOffset48
        | AbiReservedField::CopyOffset56 => Operation::Copy,
        AbiReservedField::BroadcastOffset48 | AbiReservedField::BroadcastOffset56 => {
            Operation::Broadcast
        }
        AbiReservedField::VectorOffset48 | AbiReservedField::VectorOffset56 => Operation::Vector,
    }
}

const fn reserved_offset(field: AbiReservedField) -> u64 {
    match field {
        AbiReservedField::CopyOffset40 => 40,
        AbiReservedField::CopyOffset48
        | AbiReservedField::BroadcastOffset48
        | AbiReservedField::VectorOffset48 => 48,
        AbiReservedField::CopyOffset56
        | AbiReservedField::BroadcastOffset56
        | AbiReservedField::VectorOffset56 => 56,
    }
}

const fn scalar_operation(field: AbiScalarField) -> Operation {
    match field {
        AbiScalarField::CopyLength => Operation::Copy,
        AbiScalarField::StridedRows
        | AbiScalarField::StridedRowBytes
        | AbiScalarField::StridedSourceStride
        | AbiScalarField::StridedDestinationStride => Operation::Strided,
        AbiScalarField::BroadcastLength
        | AbiScalarField::BroadcastDestinationCount
        | AbiScalarField::BroadcastListAlignment => Operation::Broadcast,
        AbiScalarField::VectorLength => Operation::Vector,
    }
}

const fn scalar_rank(field: AbiScalarField) -> u64 {
    match field {
        AbiScalarField::CopyLength
        | AbiScalarField::StridedRows
        | AbiScalarField::BroadcastLength => 32,
        AbiScalarField::StridedRowBytes
        | AbiScalarField::BroadcastDestinationCount
        | AbiScalarField::VectorLength => 40,
        AbiScalarField::StridedSourceStride | AbiScalarField::BroadcastListAlignment => 48,
        AbiScalarField::StridedDestinationStride => 56,
    }
}

const fn operand_operation(visit: AbiOperandVisit) -> Operation {
    match visit {
        AbiOperandVisit::CopySource | AbiOperandVisit::CopyDestination => Operation::Copy,
        AbiOperandVisit::StridedSource { .. } | AbiOperandVisit::StridedDestination { .. } => {
            Operation::Strided
        }
        AbiOperandVisit::BroadcastSource
        | AbiOperandVisit::BroadcastList
        | AbiOperandVisit::BroadcastDestination { .. } => Operation::Broadcast,
        AbiOperandVisit::VectorSourceA
        | AbiOperandVisit::VectorSourceB
        | AbiOperandVisit::VectorDestination => Operation::Vector,
    }
}

const fn operand_rank(visit: AbiOperandVisit) -> (u64, u64, u8) {
    match visit {
        AbiOperandVisit::CopySource
        | AbiOperandVisit::BroadcastSource
        | AbiOperandVisit::VectorSourceA => (0, 0, 0),
        AbiOperandVisit::CopyDestination | AbiOperandVisit::VectorSourceB => (0, 0, 1),
        AbiOperandVisit::StridedSource { row } => (0, row, 0),
        AbiOperandVisit::StridedDestination { row } => (0, row, 1),
        AbiOperandVisit::BroadcastList => (1, 0, 0),
        AbiOperandVisit::BroadcastDestination { index } => (2, index, 0),
        AbiOperandVisit::VectorDestination => (0, 0, 2),
    }
}

const fn operand_context(visit: AbiOperandVisit) -> (AbiEventStage, AbiEventAccess, u64) {
    match visit {
        AbiOperandVisit::CopySource
        | AbiOperandVisit::BroadcastSource
        | AbiOperandVisit::VectorSourceA => (AbiEventStage::Source, AbiEventAccess::Read, 0),
        AbiOperandVisit::VectorSourceB => (AbiEventStage::Source, AbiEventAccess::Read, 1),
        AbiOperandVisit::CopyDestination | AbiOperandVisit::VectorDestination => {
            (AbiEventStage::Destination, AbiEventAccess::Write, 0)
        }
        AbiOperandVisit::StridedSource { row } => {
            (AbiEventStage::Source, AbiEventAccess::Read, row)
        }
        AbiOperandVisit::StridedDestination { row }
        | AbiOperandVisit::BroadcastDestination { index: row } => {
            (AbiEventStage::Destination, AbiEventAccess::Write, row)
        }
        AbiOperandVisit::BroadcastList => (AbiEventStage::IndirectList, AbiEventAccess::Read, 0),
    }
}

const fn step_rank(step: AbiOperandFaultStep) -> u8 {
    match step {
        AbiOperandFaultStep::CheckedStart => 0,
        AbiOperandFaultStep::CheckedEnd => 1,
        AbiOperandFaultStep::Unmapped => 2,
        AbiOperandFaultStep::OutOfBounds => 3,
    }
}
