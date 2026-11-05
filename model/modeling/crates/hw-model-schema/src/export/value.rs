//! Recursive value callbacks shared by independent domain emitters.
mod documentation;
use super::OutputMetadata;
use crate::traversal::{Context, Visit};
use crate::{
    BitsFieldSpec, EnumDefinition, EnumVariantSpec, RecordSpec,
    ast::{BitsDefinition, LayoutExpr, value::Node},
};
pub use documentation::{
    DocumentationFragment, DocumentationKind, DocumentationOwner, documentation_fragments,
    emit_documentation,
};
/// Target handling of values in one independently chosen source domain.
pub trait Backend: Sized {
    /// Independently selected source domain.
    type Domain: super::Domain<Self>;
    /// Target-owned failure.
    type Error;
    /// Domain-specific immutable recursion context.
    type Context<'a>: Context;
    /// Identifies one root before its output writer is opened.
    fn metadata(&mut self, ctx: &Self::Context<'_>) -> Result<OutputMetadata, Self::Error>;
    /// Dispatches one framework-bound visit.
    fn emit(
        &mut self,
        visit: Visit<Self::Context<'_>>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        <Self::Domain as super::Domain<Self>>::emit(visit, self, writer)
    }
    /// Dispatches bound visits in target-selected order, stopping at the first error.
    fn emit_all<'a, I>(
        &mut self,
        visits: I,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Visit<Self::Context<'a>>>,
    {
        for visit in visits {
            self.emit(visit, writer)?;
        }
        Ok(())
    }
    /// Handles reusable or inline bits.
    fn bits<'a>(
        &mut self,
        _value: &'a BitsDefinition,
        ctx: Self::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        self.emit_all(ctx.children(), writer)
    }
    /// Handles an enumeration and its variants.
    fn enumeration(
        &mut self,
        _value: EnumDefinition,
        ctx: Self::Context<'_>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        self.emit_all(ctx.children(), writer)
    }
    /// Handles one bit field.
    fn bit_field<'a>(
        &mut self,
        _value: &'a BitsFieldSpec,
        ctx: Self::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        self.emit_all(ctx.children(), writer)
    }
    /// Handles one enum alternative.
    fn variant(
        &mut self,
        _: &EnumVariantSpec,
        _: Self::Context<'_>,
        _: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Handles a mathematical expression and recursively visits operands by default.
    fn expression<'a>(
        &mut self,
        _value: &'a LayoutExpr,
        ctx: Self::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        self.emit_all(ctx.children(), writer)
    }
    /// Handles an opaque byte envelope shared by both source domains.
    fn opaque(
        &mut self,
        _: &RecordSpec,
        _: Self::Context<'_>,
        _: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Handles a complete short description before its semantic owner callback.
    fn short_description(
        &mut self,
        _: &str,
        _: DocumentationOwner<'_>,
        _: Self::Context<'_>,
        _: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Handles a complete detailed description before its semantic owner callback.
    fn description(
        &mut self,
        _: &str,
        _: DocumentationOwner<'_>,
        _: Self::Context<'_>,
        _: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}
/// Dispatches a shared semantic value node.
///
/// # Errors
/// Propagates the target error.
pub(super) fn emit<'a, B: Backend>(
    node: Node<'a>,
    ctx: B::Context<'a>,
    backend: &mut B,
    writer: &mut dyn std::io::Write,
) -> Result<(), B::Error> {
    let documentation = match node {
        Node::Opaque(v) => Some((v.documentation(), v.name(), DocumentationKind::Opaque)),
        Node::BitField(v) => Some((v.documentation(), v.name(), DocumentationKind::BitField)),
        Node::Variant(v) => Some((v.documentation(), v.name(), DocumentationKind::Variant)),
        Node::Bits(_) | Node::Enum(_) | Node::Expression(_) => None,
    };
    if let Some((text, name, kind)) = documentation {
        emit_documentation(
            text,
            DocumentationOwner { name, kind },
            &ctx,
            backend,
            writer,
        )?;
    }
    match node {
        Node::Opaque(value) => backend.opaque(value, ctx, writer),
        Node::Bits(value) => backend.bits(value, ctx, writer),
        Node::Enum(value) => backend.enumeration(value, ctx, writer),
        Node::BitField(value) => backend.bit_field(value, ctx, writer),
        Node::Variant(value) => backend.variant(value, ctx, writer),
        Node::Expression(value) => backend.expression(value, ctx, writer),
    }
}
