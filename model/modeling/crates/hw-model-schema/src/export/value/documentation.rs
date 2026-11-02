use super::Backend;
use crate::Documentation;
/// Semantic owner of documentation, independent of target formatting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentationKind {
    /// Reusable definition.
    Definition,
    /// Record declaration.
    Record,
    /// Union declaration.
    Union,
    /// Logical record field.
    Field,
    /// Union alternative.
    Case,
    /// Register declaration.
    Register,
    /// Map declaration.
    Map,
    /// Placed register, block or bank.
    Placement,
    /// Bit field.
    BitField,
    /// Enumeration alternative.
    Variant,
    /// Opaque byte envelope.
    Opaque,
}
/// Name and semantic owner; the accompanying context retains the source definition.
#[derive(Debug, Clone, Copy)]
pub struct DocumentationOwner<'a> {
    /// Source name, without target spelling.
    pub name: &'a str,
    /// Owner category.
    pub kind: DocumentationKind,
}
/// Whole source text; target writers decide how to split and escape it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentationFragment {
    /// Concise summary.
    ShortDescription(&'static str),
    /// Full description.
    Description(&'static str),
}
/// Iterates present metadata in summary/detail order, retaining explicit empty text.
pub fn documentation_fragments(
    documentation: Documentation,
) -> impl Iterator<Item = DocumentationFragment> {
    documentation
        .short_description
        .map(DocumentationFragment::ShortDescription)
        .into_iter()
        .chain(
            documentation
                .description
                .map(DocumentationFragment::Description),
        )
}
/// Dispatches both documentation fields without imposing comment syntax.
///
/// # Errors
/// Stops on the first target error.
pub fn emit_documentation<B: Backend>(
    documentation: Documentation,
    owner: DocumentationOwner<'_>,
    ctx: &B::Context<'_>,
    backend: &mut B,
) -> Result<(), B::Error> {
    for fragment in documentation_fragments(documentation) {
        match fragment {
            DocumentationFragment::ShortDescription(text) => {
                backend.short_description(text, owner, ctx.clone())?;
            }
            DocumentationFragment::Description(text) => {
                backend.description(text, owner, ctx.clone())?;
            }
        }
    }
    Ok(())
}
