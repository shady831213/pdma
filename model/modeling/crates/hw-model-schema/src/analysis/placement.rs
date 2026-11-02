use super::{BankView, CompletePlacementLayout, RegisterView};
use crate::ast::RegisterLocation;
/// A concrete member or repeated block in a checked permission view.
#[derive(Debug, Clone, Copy)]
pub enum ConcretePlacement<'a> {
    /// Register with explicit position for naming.
    Register {
        /// Effective register.
        view: &'a RegisterView,
        /// Structural position.
        location: RegisterLocation,
    },
    /// Repeated register block.
    Bank {
        /// Checked bank.
        view: &'a BankView,
        /// Declaration position.
        index: usize,
        /// Leaves are already expanded at root.
        expanded: bool,
    },
}
/// Placement context for completed semantic traversal.
#[derive(Debug, Clone, Copy)]
pub enum Placement<'a> {
    /// Closed geometry retaining the original child reference.
    Symbolic(&'a CompletePlacementLayout),
    /// Checked concrete topology.
    Concrete(ConcretePlacement<'a>),
}
