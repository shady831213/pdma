use pdma_model_contract::semantic::{
    BroadcastCopyRequest, Command, CommandOperation, ResourceConfig, StridedCopyRequest,
};

#[derive(Debug, Clone, Copy)]
pub(super) struct ResourceDemand {
    internal_descriptors: usize,
    read_slots: usize,
    write_slots: usize,
    compute_slots: usize,
}

impl ResourceDemand {
    pub(super) fn for_command(command: &Command) -> Self {
        match command.operation() {
            CommandOperation::Copy(_) => Self::copy(),
            CommandOperation::StridedCopy(operation) => Self::strided(*operation),
            CommandOperation::BroadcastCopy(operation) => Self::broadcast(operation),
            CommandOperation::VectorAdd(_) => Self::vector_add(),
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

    pub(super) const fn strided(operation: StridedCopyRequest) -> Self {
        Self {
            internal_descriptors: operation.rows(),
            read_slots: operation.rows(),
            write_slots: operation.rows(),
            compute_slots: 0,
        }
    }

    pub(super) fn broadcast(operation: &BroadcastCopyRequest) -> Self {
        let destinations = operation.destinations().len();
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

    pub(super) const fn fits(self, config: ResourceConfig) -> bool {
        self.internal_descriptors <= config.internal_descriptors()
            && self.read_slots <= config.read_slots()
            && self.write_slots <= config.write_slots()
            && self.compute_slots <= config.compute_slots()
    }
}
