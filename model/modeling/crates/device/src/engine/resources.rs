use pdma_model_contract::semantic::{Command, CommandOperation, ResourceConfig};

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
            CommandOperation::Copy(_) => Self {
                internal_descriptors: 1,
                read_slots: 1,
                write_slots: 1,
                compute_slots: 0,
            },
            CommandOperation::StridedCopy(operation) => Self {
                internal_descriptors: operation.rows(),
                read_slots: operation.rows(),
                write_slots: operation.rows(),
                compute_slots: 0,
            },
            CommandOperation::BroadcastCopy(operation) => Self {
                internal_descriptors: operation.destinations().len(),
                read_slots: operation.destinations().len(),
                write_slots: operation.destinations().len(),
                compute_slots: 0,
            },
            CommandOperation::VectorAdd(_) => Self {
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
