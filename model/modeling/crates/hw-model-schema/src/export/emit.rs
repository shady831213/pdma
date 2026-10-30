use super::Backend;
use crate::ast::{
    Field, Node, PlacementView, RegisterInfo, RegisterLocation,
    analysis::{Analysis, Layout, MapLayout, Unit},
};

/// Immutable information for one recursive call; no mutable render stack is kept.
#[derive(Debug, Clone, Copy)]
pub struct Scope<'a> {
    /// Common analysis and the selected definition set.
    pub analysis: &'a Analysis,
    /// Current definition or named projection.
    pub unit: &'a Unit,
    /// Enclosing logical field, if any.
    pub field: Option<&'a Field>,
    /// Current concrete or declared placement, if any.
    pub placement: Option<PlacementView<'a>>,
    /// Enclosing register, if any.
    pub register: Option<&'a RegisterInfo>,
    /// Enclosing bit-field name for enum rendering.
    pub bit: Option<&'static str>,
}
impl<'a> Scope<'a> {
    /// Constructs the root call's scope.
    pub const fn root(analysis: &'a Analysis, unit: &'a Unit) -> Self {
        Self {
            analysis,
            unit,
            field: None,
            placement: None,
            register: None,
            bit: None,
        }
    }
    /// Checked layout of the enclosing map, if this unit describes one.
    pub const fn map_layout(self) -> Option<&'a MapLayout> {
        if let Layout::Map(map) = &self.unit.body {
            Some(map)
        } else {
            None
        }
    }
    /// Concrete position used for target names and instance metadata.
    pub const fn register_location(self) -> RegisterLocation {
        match self.placement {
            Some(PlacementView::Register { location, .. }) => location,
            Some(PlacementView::Declared(_) | PlacementView::Bank { .. }) | None => {
                RegisterLocation::Root
            }
        }
    }
}

/// Dispatches one original schema node with recursion-local scope.
///
/// # Errors
/// Propagates the target's error without emitting artifacts to storage.
pub fn emit<'a, B: Backend>(
    node: Node<'a>,
    mut scope: Scope<'a>,
    backend: &mut B,
) -> Result<(), B::Error> {
    match node {
        Node::Record(value) => backend.record(value, scope),
        Node::Register(value) => {
            scope.register = Some(value);
            backend.register(value, scope)
        }
        Node::RegisterMap(value) => backend.register_map(value, scope),
        Node::Bits(value) => backend.bits(value, scope),
        Node::Enum(value) => backend.enumeration(value, scope),
        Node::Union(value) => backend.union(value, scope),
        Node::Field(value) => {
            scope.field = Some(value);
            backend.field(value, scope)
        }
        Node::BitField(value) => {
            scope.bit = Some(value.name());
            backend.bit_field(value, scope)
        }
        Node::RegisterField(value) => {
            scope.bit = Some(value.name());
            backend.register_field(value, scope)
        }
        Node::Case(value) => backend.case(value, scope),
        Node::Variant(value) => backend.variant(value, scope),
        Node::Placement(value) => {
            scope.placement = Some(value);
            backend.placement(value, scope)
        }
        Node::Opaque(value) => backend.opaque(value, scope),
    }
}

/// Emits an iterator in its chosen order; callers may supply filtered or sorted views.
///
/// # Errors
/// Stops at the first target error.
pub fn emit_all<'a, B: Backend>(
    nodes: impl IntoIterator<Item = Node<'a>>,
    scope: Scope<'a>,
    backend: &mut B,
) -> Result<(), B::Error> {
    for node in nodes {
        emit(node, scope, backend)?;
    }
    Ok(())
}
