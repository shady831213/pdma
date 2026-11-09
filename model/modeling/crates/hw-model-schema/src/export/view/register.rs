use super::Reference;
use crate::{analysis, ast};

/// Completed bank metadata without access to its register topology.
#[derive(Clone, Copy)]
pub struct Bank<'a>(pub(crate) crate::traversal::register::Bank<'a>);
impl<'a> Bank<'a> {
    /// Stable bank position in the selected map.
    pub const fn index(self) -> usize {
        self.0.index()
    }
    /// Original placement name.
    pub const fn name(self) -> &'static str {
        self.0.name()
    }
    /// Source path of this bank.
    pub fn path(self) -> &'a str {
        self.0.path()
    }
    /// Structural source path components.
    pub fn segments(self) -> &'a [analysis::ViewPathSegment] {
        self.0.segments()
    }
    /// Placement documentation.
    pub const fn documentation(self) -> crate::Documentation {
        self.0.documentation()
    }
}
impl core::fmt::Debug for Bank<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Bank")
            .field("index", &self.index())
            .field("name", &self.name())
            .finish_non_exhaustive()
    }
}

/// Borrowed register metadata without contained declarations.
#[derive(Clone, Copy)]
pub struct Register<'a>(pub(crate) &'a ast::RegisterInfo);
impl Register<'_> {
    /// Whether semantic bit fields were declared.
    pub fn has_fields(self) -> bool {
        !self.0.fields().is_empty()
    }
    /// Returns the name.
    pub const fn name(self) -> &'static str {
        self.0.name()
    }
    /// Returns the documentation.
    pub const fn documentation(self) -> crate::Documentation {
        self.0.documentation()
    }
    /// Returns the width.
    pub const fn width(self) -> usize {
        self.0.width()
    }
    /// Returns the alignment.
    pub const fn alignment(self) -> usize {
        self.0.alignment()
    }
    /// Returns the endian.
    pub const fn endian(self) -> crate::Endian {
        self.0.endian()
    }
    /// Returns the access.
    pub const fn access(self) -> crate::Access {
        self.0.access()
    }
    /// Returns the reset kind.
    pub const fn reset_kind(self) -> crate::ResetKind {
        self.0.reset_kind()
    }
    /// Returns the reset value.
    pub const fn reset_value(self) -> u64 {
        self.0.reset_value()
    }
    /// Returns the writable mask.
    pub const fn writable_mask(self) -> u64 {
        self.0.writable_mask()
    }
}
/// Borrowed map metadata without contained declarations.
#[derive(Clone, Copy)]
pub struct Map<'a>(pub(crate) &'a analysis::RegisterMapView);
impl Map<'_> {
    /// Returns the extent.
    pub const fn extent(self) -> usize {
        self.0.extent
    }
    /// Returns the alignment.
    pub const fn alignment(self) -> Option<usize> {
        self.0.alignment
    }
    /// Returns the documentation.
    pub const fn documentation(self) -> crate::Documentation {
        self.0.documentation
    }
}
/// Borrowed map layout metadata without contained declarations.
#[derive(Clone, Copy)]
pub struct MapLayout<'a>(pub(crate) &'a analysis::CompleteMapLayout);
impl MapLayout<'_> {
    /// Returns the alignment.
    pub const fn alignment(self) -> usize {
        self.0.alignment()
    }
    /// Returns the parameters.
    pub const fn parameters(self) -> &'static [&'static str] {
        self.0.parameters()
    }
    /// Returns the source parameters.
    pub const fn source_parameters(self) -> &'static [&'static str] {
        self.0.source_parameters()
    }
}
/// Borrowed placement metadata without contained declarations.
#[derive(Clone, Copy)]
pub struct Placement<'a>(pub(crate) &'a analysis::CompletePlacementLayout);
impl Placement<'_> {
    /// Returns the name.
    pub const fn name(self) -> &'static str {
        self.0.source.name()
    }
    /// Returns the is bank.
    pub const fn is_bank(self) -> bool {
        self.0.bank
    }
    /// Returns the documentation.
    pub const fn documentation(self) -> crate::Documentation {
        self.0.documentation
    }
    /// Returns the child alignment.
    pub const fn child_alignment(self) -> usize {
        self.0.child_alignment
    }
}
/// Borrowed register placement metadata without contained declarations.
#[derive(Clone, Copy)]
pub struct RegisterPlacement<'a>(pub(crate) &'a analysis::RegisterView);
impl RegisterPlacement<'_> {
    /// Returns the name.
    pub const fn name(self) -> &'static str {
        self.0.name
    }
    /// Returns the offset.
    pub const fn offset(self) -> usize {
        self.0.offset
    }
    /// Returns the documentation.
    pub const fn documentation(self) -> crate::Documentation {
        self.0.documentation
    }
}
impl<'a> MapLayout<'a> {
    /// Concrete root bindings, if selected.
    pub fn bindings(self) -> Option<&'a [usize]> {
        self.0.bindings()
    }
}
impl<'a> RegisterPlacement<'a> {
    /// Root-qualified source path.
    pub fn path(self) -> &'a str {
        &self.0.path
    }
    /// Structural source path components.
    pub fn segments(self) -> &'a [analysis::ViewPathSegment] {
        &self.0.segments
    }
}
impl<'a> Register<'a> {
    /// Referenced source bits metadata, absent for inline field declarations.
    pub fn source(self) -> Option<Reference<'a>> {
        self.0.source().map(Reference)
    }
}

impl core::fmt::Debug for Register<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Register")
            .field("name", &self.name())
            .finish_non_exhaustive()
    }
}

impl core::fmt::Debug for Map<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Map")
            .field("extent", &self.extent())
            .finish_non_exhaustive()
    }
}

impl core::fmt::Debug for MapLayout<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MapLayout")
            .field("alignment", &self.alignment())
            .finish_non_exhaustive()
    }
}

impl core::fmt::Debug for Placement<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Placement")
            .field("name", &self.name())
            .finish_non_exhaustive()
    }
}

impl core::fmt::Debug for RegisterPlacement<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("RegisterPlacement")
            .field("name", &self.name())
            .field("offset", &self.offset())
            .finish_non_exhaustive()
    }
}
