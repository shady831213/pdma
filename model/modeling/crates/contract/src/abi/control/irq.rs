use crate::{IrqRoute, IrqSource};

use super::DeviceEpoch;

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
