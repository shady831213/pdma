//! Public resource-credit configuration shared across the model boundary.

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
    /// Internal descriptor capacity.
    pub const fn internal_descriptors(self) -> usize {
        self.internal_descriptors
    }

    /// Read slot capacity.
    pub const fn read_slots(self) -> usize {
        self.read_slots
    }

    /// Write slot capacity.
    pub const fn write_slots(self) -> usize {
        self.write_slots
    }

    /// Compute slot capacity.
    pub const fn compute_slots(self) -> usize {
        self.compute_slots
    }

    /// Completion publication slot capacity.
    pub const fn completion_publication_slots(self) -> usize {
        self.completion_publication_slots
    }

    /// Event publication slot capacity.
    pub const fn event_publication_slots(self) -> usize {
        self.event_publication_slots
    }

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
