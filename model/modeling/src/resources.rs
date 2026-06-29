//! Execution resource-credit configuration and accounting.

use crate::desc::ScheduledWork;

/// Device execution resource capacities used by the deterministic scheduler gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceConfig {
    internal_descriptors: usize,
    read_slots: usize,
    write_slots: usize,
    compute_slots: usize,
    completion_publication_slots: usize,
    event_publication_slots: usize,
}

impl ResourceConfig {
    /// Returns a copy with a new internal descriptor capacity.
    #[must_use]
    pub const fn with_internal_descriptors(self, internal_descriptors: usize) -> Self {
        Self {
            internal_descriptors,
            ..self
        }
    }

    /// Returns a copy with a new read slot capacity.
    #[must_use]
    pub const fn with_read_slots(self, read_slots: usize) -> Self {
        Self { read_slots, ..self }
    }

    /// Returns a copy with a new write slot capacity.
    #[must_use]
    pub const fn with_write_slots(self, write_slots: usize) -> Self {
        Self {
            write_slots,
            ..self
        }
    }

    /// Returns a copy with a new compute slot capacity.
    #[must_use]
    pub const fn with_compute_slots(self, compute_slots: usize) -> Self {
        Self {
            compute_slots,
            ..self
        }
    }

    /// Returns a copy with a new completion publication capacity.
    #[must_use]
    pub const fn with_completion_publication_slots(
        self,
        completion_publication_slots: usize,
    ) -> Self {
        Self {
            completion_publication_slots,
            ..self
        }
    }

    /// Returns a copy with a new event publication capacity.
    #[must_use]
    pub const fn with_event_publication_slots(self, event_publication_slots: usize) -> Self {
        Self {
            event_publication_slots,
            ..self
        }
    }
}

impl Default for ResourceConfig {
    fn default() -> Self {
        Self {
            internal_descriptors: usize::MAX,
            read_slots: usize::MAX,
            write_slots: usize::MAX,
            compute_slots: usize::MAX,
            completion_publication_slots: usize::MAX,
            event_publication_slots: usize::MAX,
        }
    }
}

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
        self.available.internal_descriptors >= demand.internal_descriptors
            && self.available.read_slots >= demand.read_slots
            && self.available.write_slots >= demand.write_slots
            && self.available.compute_slots >= demand.compute_slots
    }

    pub(crate) const fn has_event_publication_slot(self, occupied: usize) -> bool {
        occupied < self.available.event_publication_slots
    }

    pub(crate) const fn has_completion_publication_slot(self, occupied: usize) -> bool {
        occupied < self.available.completion_publication_slots
    }

    pub(crate) const fn acquire(&mut self, demand: ResourceDemand) {
        self.available.internal_descriptors -= demand.internal_descriptors;
        self.available.read_slots -= demand.read_slots;
        self.available.write_slots -= demand.write_slots;
        self.available.compute_slots -= demand.compute_slots;
    }

    pub(crate) const fn release(&mut self, demand: ResourceDemand) {
        self.available.internal_descriptors += demand.internal_descriptors;
        self.available.read_slots += demand.read_slots;
        self.available.write_slots += demand.write_slots;
        self.available.compute_slots += demand.compute_slots;
    }
}
