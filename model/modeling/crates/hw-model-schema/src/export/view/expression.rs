use crate::ast::{LayoutError, LayoutExpr};

/// A complete expression evaluated or formatted through the shared semantic fold.
#[derive(Clone, Copy)]
pub struct Expression<'a>(pub(crate) &'a LayoutExpr);
impl Expression<'_> {
    /// Formats through the shared expression fold and target syntax.
    ///
    /// # Errors
    /// Rejects unresolved or unsupported semantic operands.
    pub fn format<S: crate::export::expression::ExpressionSyntax>(
        self,
        syntax: &mut S,
    ) -> Result<String, LayoutError> {
        crate::export::expression::format_expression(self.0, syntax)
    }
    /// Evaluates with the shared checked arithmetic and parameter binding rules.
    ///
    /// # Errors
    /// Rejects invalid bindings, unsupported operands, or invalid arithmetic.
    pub fn evaluate(self, parameters: &[&str], arguments: &[usize]) -> Result<usize, LayoutError> {
        self.0.evaluate(parameters, arguments)
    }
}
/// Child constructor binding without raw expression access.
#[derive(Clone, Copy)]
pub struct Argument<'a>(pub(crate) crate::ast::register::Argument<'a>);
impl Argument<'_> {
    /// Child parameter name.
    pub const fn parameter(self) -> Option<&'static str> {
        self.0.parameter
    }
    /// Constructor position.
    pub const fn index(self) -> usize {
        self.0.index
    }
}
/// Completed layout formula and its scalar ownership metadata.
#[derive(Clone, Copy)]
pub struct LayoutProperty<'a>(pub(crate) crate::traversal::register::LayoutProperty<'a>);
impl<'a> LayoutProperty<'a> {
    /// Relative owner, absent for map-owned properties.
    pub const fn owner(self) -> Option<&'a str> {
        self.0.owner
    }
    /// Semantic property name.
    pub const fn name(self) -> &'static str {
        self.0.name
    }
    /// Semantic property classification.
    pub const fn kind(self) -> crate::traversal::register::LayoutPropertyKind {
        self.0.kind
    }
}

impl core::fmt::Debug for Expression<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Expression").finish_non_exhaustive()
    }
}

impl core::fmt::Debug for Argument<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Argument")
            .field("parameter", &self.parameter())
            .field("index", &self.index())
            .finish_non_exhaustive()
    }
}

impl core::fmt::Debug for LayoutProperty<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("LayoutProperty")
            .field("owner", &self.owner())
            .field("name", &self.name())
            .finish_non_exhaustive()
    }
}
