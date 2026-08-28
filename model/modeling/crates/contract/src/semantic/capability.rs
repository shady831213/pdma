//! Epoch-qualified semantic device capabilities.

use crate::semantic::{IrqRoute, IrqSource, QueueId, SemanticError, SemanticResult};

/// Device generation attached to resettable capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DeviceEpoch(u64);

impl DeviceEpoch {
    /// Initial generation before the first whole-device reset.
    pub const fn initial() -> Self {
        Self(0)
    }

    /// Creates a generation from a complete fixed-width value.
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// Complete generation value.
    pub const fn raw(self) -> u64 {
        self.0
    }

    /// Advances a generation without wrapping.
    pub const fn next(self) -> SemanticResult<Self> {
        match self.0.checked_add(1) {
            Some(next) => Ok(Self(next)),
            None => Err(SemanticError::DeviceEpochOverflow { current: self.0 }),
        }
    }
}

/// Queue identity paired with the generation in which it is valid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EpochQueue {
    epoch: DeviceEpoch,
    queue_id: QueueId,
}

impl EpochQueue {
    /// Creates an epoch-qualified queue identity.
    pub const fn new(epoch: DeviceEpoch, queue_id: QueueId) -> Self {
        Self { epoch, queue_id }
    }

    /// Owning device generation.
    pub const fn epoch(self) -> DeviceEpoch {
        self.epoch
    }

    /// Queue identity within the generation.
    pub const fn queue_id(self) -> QueueId {
        self.queue_id
    }
}

/// PDMA-local IRQ source paired with its owning device generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EpochIrqSource {
    epoch: DeviceEpoch,
    source: IrqSource,
}

impl EpochIrqSource {
    /// Creates an epoch-qualified IRQ source.
    pub const fn new(epoch: DeviceEpoch, source: IrqSource) -> Self {
        Self { epoch, source }
    }

    /// Owning device generation.
    pub const fn epoch(self) -> DeviceEpoch {
        self.epoch
    }

    /// PDMA-local source within the generation.
    pub const fn source(self) -> IrqSource {
        self.source
    }
}

/// Epoch-qualified route assignment for one PDMA-local source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EpochIrqRoute {
    source: EpochIrqSource,
    route: IrqRoute,
}

impl EpochIrqRoute {
    /// Creates a route assignment from typed source and route values.
    pub const fn new(source: EpochIrqSource, route: IrqRoute) -> Self {
        Self { source, route }
    }

    /// Epoch-qualified source.
    pub const fn source(self) -> EpochIrqSource {
        self.source
    }

    /// Host delivery route.
    pub const fn route(self) -> IrqRoute {
        self.route
    }
}

/// Typed PDMA-local IRQ route, control, and observation operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrqControl {
    /// Assign a route while the source is inactive.
    ConfigureRoute(EpochIrqRoute),
    /// Enable a source.
    Enable(EpochIrqSource),
    /// Disable a source.
    Disable(EpochIrqSource),
    /// Mask delivery without clearing pending state.
    Mask(EpochIrqSource),
    /// Unmask delivery.
    Unmask(EpochIrqSource),
    /// Acknowledge a source.
    Acknowledge(EpochIrqSource),
    /// Observe pending state for a source.
    Observe(EpochIrqSource),
}

/// Device execution state returned by typed status discovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceRunState {
    /// No runnable or pending work.
    Idle,
    /// Work can make progress.
    Running,
    /// Work exists but cannot currently progress.
    Blocked,
}

/// Whole-device typed status response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceStatus {
    epoch: DeviceEpoch,
    run_state: DeviceRunState,
}

impl DeviceStatus {
    /// Creates a status snapshot from already typed values.
    pub const fn new(epoch: DeviceEpoch, run_state: DeviceRunState) -> Self {
        Self { epoch, run_state }
    }

    /// Snapshot generation.
    pub const fn epoch(self) -> DeviceEpoch {
        self.epoch
    }

    /// Snapshot execution state.
    pub const fn run_state(self) -> DeviceRunState {
        self.run_state
    }
}

/// Whole-device reset request guarded by an expected generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WholeDeviceReset {
    expected_epoch: DeviceEpoch,
}

impl WholeDeviceReset {
    /// Creates an epoch-guarded reset request.
    pub const fn new(expected_epoch: DeviceEpoch) -> Self {
        Self { expected_epoch }
    }

    /// Generation that must still be current before reset mutation.
    pub const fn expected_epoch(self) -> DeviceEpoch {
        self.expected_epoch
    }
}
