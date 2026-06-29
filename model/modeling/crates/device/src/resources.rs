//! Execution resource-credit configuration and accounting.

use crate::scheduled::ScheduledWork;
use pdma_model_contract::resources::ResourceConfig;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ResourceDemand {
    internal_descriptors: usize,
    read_slots: usize,
    write_slots: usize,
    compute_slots: usize,
}

impl ResourceDemand {
    pub(crate) fn for_work(work: &ScheduledWork) -> Self {
        match work {
            ScheduledWork::Copy(copy) => {
                let chunks = copy.chunks().len();
                Self {
                    internal_descriptors: chunks,
                    read_slots: chunks,
                    write_slots: chunks,
                    compute_slots: 0,
                }
            }
            ScheduledWork::VectorAdd(_) => Self {
                internal_descriptors: 1,
                read_slots: 2,
                write_slots: 1,
                compute_slots: 1,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ResourcePool {
    available: ResourceConfig,
}

impl ResourcePool {
    pub(crate) const fn new(available: ResourceConfig) -> Self {
        Self { available }
    }

    pub(crate) const fn has_available(self, demand: ResourceDemand) -> bool {
        self.available.internal_descriptors() >= demand.internal_descriptors
            && self.available.read_slots() >= demand.read_slots
            && self.available.write_slots() >= demand.write_slots
            && self.available.compute_slots() >= demand.compute_slots
    }

    pub(crate) const fn has_event_publication_slot(self, occupied: usize) -> bool {
        occupied < self.available.event_publication_slots()
    }

    pub(crate) const fn has_completion_publication_slot(self, occupied: usize) -> bool {
        occupied < self.available.completion_publication_slots()
    }

    pub(crate) const fn acquire(&mut self, demand: ResourceDemand) {
        self.available = self
            .available
            .with_internal_descriptors(
                self.available.internal_descriptors() - demand.internal_descriptors,
            )
            .with_read_slots(self.available.read_slots() - demand.read_slots)
            .with_write_slots(self.available.write_slots() - demand.write_slots)
            .with_compute_slots(self.available.compute_slots() - demand.compute_slots);
    }

    pub(crate) const fn release(&mut self, demand: ResourceDemand) {
        self.available = self
            .available
            .with_internal_descriptors(
                self.available.internal_descriptors() + demand.internal_descriptors,
            )
            .with_read_slots(self.available.read_slots() + demand.read_slots)
            .with_write_slots(self.available.write_slots() + demand.write_slots)
            .with_compute_slots(self.available.compute_slots() + demand.compute_slots);
    }
}
