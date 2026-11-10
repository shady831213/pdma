use crate::analysis::{CompleteMapLayout, RegisterMapView};
/// One map occurrence. Concrete address projection is optional derived data,
/// never a second implicit sibling in the declaration traversal.
#[derive(Debug, Clone, Copy)]
pub enum Map<'a> {
    /// A checked declaration with an optional prepared concrete address projection.
    Declaration {
        /// Closed symbolic or bound layout.
        layout: &'a CompleteMapLayout,
        /// Effective concrete registers, if the entry was fully bound.
        projection: Option<&'a RegisterMapView>,
    },
    /// An authoritative permission/address projection with no invented source declaration.
    Projection(&'a RegisterMapView),
}
impl<'a> Map<'a> {
    /// Original analysis layout, absent for a permission-only projection.
    pub const fn layout(self) -> Option<&'a CompleteMapLayout> {
        match self {
            Self::Declaration { layout, .. } => Some(layout),
            Self::Projection(_) => None,
        }
    }
    /// Explicit concrete address projection, when available.
    pub const fn projection(self) -> Option<&'a RegisterMapView> {
        match self {
            Self::Declaration { projection, .. } => projection,
            Self::Projection(v) => Some(v),
        }
    }
    /// Required alignment when known; projection does not invent missing provenance.
    pub const fn alignment(self) -> Option<usize> {
        match self {
            Self::Declaration { layout, .. } => Some(layout.alignment()),
            Self::Projection(v) => v.alignment,
        }
    }
    /// Concrete extent, absent when the declaration is not fully bound.
    pub fn extent(self) -> Option<usize> {
        match self {
            Self::Declaration { layout, .. } => layout.extent().evaluate(&[], &[]).ok(),
            Self::Projection(v) => Some(v.extent),
        }
    }
    /// Whether the input itself is a permission/address projection.
    pub const fn is_projection(self) -> bool {
        matches!(self, Self::Projection(_))
    }
    /// Free expression parameters.
    pub fn parameters(self) -> &'static [&'static str] {
        self.layout().map_or(&[], CompleteMapLayout::parameters)
    }
    /// Original map formals.
    pub fn source_parameters(self) -> &'static [&'static str] {
        self.layout()
            .map_or(&[], CompleteMapLayout::source_parameters)
    }
    /// Actual concrete argument bindings.
    pub fn bindings(self) -> Option<&'a [usize]> {
        self.layout().and_then(CompleteMapLayout::bindings)
    }
}
