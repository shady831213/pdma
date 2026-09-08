use crate::operation::{BroadcastOperands, DecodedCommand, DecodedOperation, StridedOperands};
use pdma_model_contract::semantic::ResourceConfig;

#[derive(Debug, Clone, Copy)]
pub(in crate::device) struct ResourceDemand {
    internal_descriptors: usize,
    read_slots: usize,
    write_slots: usize,
    compute_slots: usize,
}

impl ResourceDemand {
    pub(in crate::device) fn for_command(command: &DecodedCommand) -> Self {
        Self::for_operation(&command.operation)
    }

    pub(super) fn for_operation(operation: &DecodedOperation) -> Self {
        match operation {
            DecodedOperation::Copy(_) => Self::copy(),
            DecodedOperation::StridedCopy(operation) => Self::strided(operation),
            DecodedOperation::BroadcastCopy(operation) => Self::broadcast(operation),
            DecodedOperation::VectorAdd(_) => Self::vector_add(),
        }
    }

    pub(super) const fn copy() -> Self {
        Self {
            internal_descriptors: 1,
            read_slots: 1,
            write_slots: 1,
            compute_slots: 0,
        }
    }

    pub(super) fn strided(operation: &StridedOperands) -> Self {
        let rows = usize::try_from(operation.rows).unwrap_or(usize::MAX);
        Self {
            internal_descriptors: rows,
            read_slots: rows,
            write_slots: rows,
            compute_slots: 0,
        }
    }

    pub(super) fn broadcast(operation: &BroadcastOperands) -> Self {
        let destinations = usize::try_from(operation.destination_count).unwrap_or(usize::MAX);
        Self {
            internal_descriptors: destinations,
            read_slots: destinations,
            write_slots: destinations,
            compute_slots: 0,
        }
    }

    pub(super) const fn vector_add() -> Self {
        Self {
            internal_descriptors: 1,
            read_slots: 2,
            write_slots: 1,
            compute_slots: 1,
        }
    }

    pub(in crate::device) const fn fits(self, config: ResourceConfig) -> bool {
        self.internal_descriptors <= config.internal_descriptors()
            && self.read_slots <= config.read_slots()
            && self.write_slots <= config.write_slots()
            && self.compute_slots <= config.compute_slots()
    }
}
