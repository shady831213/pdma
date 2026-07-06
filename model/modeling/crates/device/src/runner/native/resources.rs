use pdma_model_contract::ResourceConfig;
use pdma_model_contract::desc::ExternalCommand;

#[derive(Debug, Clone, Copy)]
pub(super) struct ResourceDemand {
    internal_descriptors: usize,
    read_slots: usize,
    write_slots: usize,
    compute_slots: usize,
}

impl ResourceDemand {
    pub(super) fn for_command(command: &ExternalCommand) -> Self {
        match command {
            ExternalCommand::Copy(_) => Self {
                internal_descriptors: 1,
                read_slots: 1,
                write_slots: 1,
                compute_slots: 0,
            },
            ExternalCommand::StridedCopy(command) => Self {
                internal_descriptors: command.request().rows(),
                read_slots: command.request().rows(),
                write_slots: command.request().rows(),
                compute_slots: 0,
            },
            ExternalCommand::BroadcastCopy(command) => Self {
                internal_descriptors: command.request().destinations().len(),
                read_slots: command.request().destinations().len(),
                write_slots: command.request().destinations().len(),
                compute_slots: 0,
            },
            ExternalCommand::VectorAdd(_) => Self {
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
