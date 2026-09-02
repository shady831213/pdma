//! Device-local interrupt routing, masking, latching, and notification state.

#[doc(hidden)]
#[doc(hidden)]
pub(crate) mod controller;

pub(crate) use controller::PdmaIrqController;

use pdma_model_contract::semantic::IrqRoute;

#[derive(Debug, Clone, Copy)]
pub(crate) struct IrqSourceObservation {
    pub(crate) route: IrqRoute,
    pub(crate) pending_messages: usize,
    state: u8,
}

impl IrqSourceObservation {
    pub(crate) const fn new(route: IrqRoute, pending_messages: usize, state: u8) -> Self {
        Self {
            route,
            pending_messages,
            state,
        }
    }

    pub(crate) const fn enabled(self) -> bool {
        self.state & 1 != 0
    }

    pub(crate) const fn masked(self) -> bool {
        self.state & 2 != 0
    }

    pub(crate) const fn cause_pending(self) -> bool {
        self.state & 4 != 0
    }

    pub(crate) const fn deliverable(self) -> bool {
        self.state & 8 != 0
    }

    pub(crate) const fn line_asserted(self) -> bool {
        self.state & 16 != 0
    }
}
