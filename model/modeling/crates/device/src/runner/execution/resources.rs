use super::command::{ExecutionCommand, ExecutionOperation};
use pdma_model_contract::semantic::ResourceConfig;

#[derive(Debug, Clone, Copy)]
pub(super) struct ResourceDemand {
    internal_descriptors: usize,
    read_slots: usize,
    write_slots: usize,
    compute_slots: usize,
}

impl ResourceDemand {
    pub(super) fn for_command(command: &ExecutionCommand) -> Self {
        match command.operation() {
            ExecutionOperation::Copy(_) => Self {
                internal_descriptors: 1,
                read_slots: 1,
                write_slots: 1,
                compute_slots: 0,
            },
            ExecutionOperation::StridedCopy(operation) => Self {
                internal_descriptors: operation.rows.count(),
                read_slots: operation.rows.count(),
                write_slots: operation.rows.count(),
                compute_slots: 0,
            },
            ExecutionOperation::BroadcastCopy(operation) => Self {
                internal_descriptors: operation.destinations.len(),
                read_slots: operation.destinations.len(),
                write_slots: operation.destinations.len(),
                compute_slots: 0,
            },
            ExecutionOperation::VectorAdd(_) => Self {
                internal_descriptors: 1,
                read_slots: 2,
                write_slots: 1,
                compute_slots: 1,
            },
        }
    }

    pub(super) const fn fits(self, config: ResourceConfig) -> bool {
        self.internal_descriptors <= config.internal_descriptors()
            && self.read_slots <= config.read_slots()
            && self.write_slots <= config.write_slots()
            && self.compute_slots <= config.compute_slots()
    }
}
