mod selection;
mod types;

pub use selection::{AbiFaultCandidate, select_canonical_fault};
pub use types::{AbiFaultOrder, AbiFaultSelectionError, AbiOperandFaultStep, AbiOperandVisit};

use crate::abi::descriptor::{
    DescriptorHeaderSchemaOpcode, DescriptorViolationKind, header_rank, location_offset,
    scalar_opcode, scalar_rank,
};
use crate::abi::fault::{AbiEventAccess, AbiEventStage};
use types::OrderKey;

fn operation_for(order: AbiFaultOrder) -> Option<DescriptorHeaderSchemaOpcode> {
    match order {
        AbiFaultOrder::Descriptor(kind) => descriptor_operation(kind),
        AbiFaultOrder::Operand { visit, .. } => Some(operand_operation(visit)),
    }
}

fn order_key(order: AbiFaultOrder) -> OrderKey {
    match order {
        AbiFaultOrder::Descriptor(kind) => descriptor_key(kind),
        AbiFaultOrder::Operand { visit, step } => {
            let (major, index, side) = operand_rank(visit);
            OrderKey::new(3, major, index, side, step_rank(step))
        }
    }
}

fn descriptor_operation(kind: DescriptorViolationKind) -> Option<DescriptorHeaderSchemaOpcode> {
    match kind {
        DescriptorViolationKind::Header(_) => None,
        DescriptorViolationKind::Reserved { opcode, .. } => Some(opcode),
        DescriptorViolationKind::Scalar(location) => scalar_opcode(location),
    }
}

fn descriptor_key(kind: DescriptorViolationKind) -> OrderKey {
    match kind {
        DescriptorViolationKind::Header(location) => {
            let rank = header_rank(location).unwrap_or_else(|| location_offset(location));
            OrderKey::new(0, rank, 0, 0, 0)
        }
        DescriptorViolationKind::Reserved { location, .. } => {
            OrderKey::new(1, location_offset(location), 0, 0, 0)
        }
        DescriptorViolationKind::Scalar(location) => {
            let rank = scalar_rank(location).unwrap_or_else(|| location_offset(location));
            OrderKey::new(2, rank, 0, 0, 0)
        }
    }
}

const fn operand_operation(visit: AbiOperandVisit) -> DescriptorHeaderSchemaOpcode {
    match visit {
        AbiOperandVisit::CopySource | AbiOperandVisit::CopyDestination => {
            DescriptorHeaderSchemaOpcode::Copy
        }
        AbiOperandVisit::StridedSource { .. } | AbiOperandVisit::StridedDestination { .. } => {
            DescriptorHeaderSchemaOpcode::StridedCopy
        }
        AbiOperandVisit::BroadcastSource
        | AbiOperandVisit::BroadcastList
        | AbiOperandVisit::BroadcastDestination { .. } => {
            DescriptorHeaderSchemaOpcode::BroadcastCopy
        }
        AbiOperandVisit::VectorSourceA
        | AbiOperandVisit::VectorSourceB
        | AbiOperandVisit::VectorDestination => DescriptorHeaderSchemaOpcode::VectorAdd,
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
