//! Recursive value callbacks shared by independent domain emitters.
mod documentation;
use super::{Context, Domain, Visit, view};
use crate::{EnumVariantSpec, ast::value::Node};
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
    /// Optional target naming/validity check, run for every entry before writing.
    /// Fragment-only renderers need no output metadata or naming implementation.
    fn check_entry(
        &mut self,
        _: &Self::Context<'_>,
        _: &mut super::EntrySymbols,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Handles a scalar value and any packed field declarations.
    fn scalar(
        &mut self,
        _: view::Scalar,
        ctx: Self::Context<'_>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        self.emit_all(ctx.children(), writer)
    }
    /// Handles a canonical physical field selected by a domain projection.
    fn storage_field<'a>(
        &mut self,
        _: view::StorageField<'a>,
        ctx: Self::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        self.emit_all(ctx.children(), writer)
    }
    /// Handles a completed opaque member and its enum or bit children.
    fn storage_member<'a>(
        &mut self,
        _value: view::StorageMember<'a>,
        ctx: Self::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        self.emit_all(ctx.children(), writer)
    }
    /// Handles reusable or inline bits.
    fn bits<'a>(
        &mut self,
        _value: view::Bits<'a>,
        ctx: Self::Context<'a>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        self.emit_all(ctx.children(), writer)
    }
    /// Handles an enumeration and its variants.
    fn enumeration(
        &mut self,
        _value: view::Enumeration,
        ctx: Self::Context<'_>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        self.emit_all(ctx.children(), writer)
    }
    /// Handles one bit field.
    fn bit_field(
        &mut self,
        _value: view::BitField,
        ctx: Self::Context<'_>,
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
    /// Handles a complete mathematical expression; defaults to no output.
    fn expression<'a>(
        &mut self,
        _value: view::Expression<'a>,
        _: Self::Context<'a>,
        _: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Handles an opaque byte envelope shared by both source domains.
    fn opaque(
        &mut self,
        _: view::Storage,
        ctx: Self::Context<'_>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        self.emit_all(ctx.children(), writer)
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
    /// Handles detailed documentation; defaults to the short-description formatter.
    /// Override independently when summaries and detailed text need different output.
    fn description(
        &mut self,
        text: &str,
        owner: DocumentationOwner<'_>,
        ctx: Self::Context<'_>,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), Self::Error> {
        self.short_description(text, owner, ctx, writer)
    }
}
/// Shared dispatch cannot be replaced by a target implementation.
pub trait Emit: Backend {
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
}
impl<B: Backend> Emit for B {}

/// Dispatches a shared semantic value node.
///
/// # Errors
/// Propagates the target error.
pub(super) fn emit<'a, B: Backend>(
    node: Node<'a>,
    endian: crate::Endian,
    ctx: B::Context<'a>,
    backend: &mut B,
    writer: &mut dyn std::io::Write,
) -> Result<(), B::Error> {
    let documentation = match node {
        Node::Opaque(v) => Some((v.documentation(), v.name(), DocumentationKind::Opaque)),
        Node::BitField(v) => Some((v.documentation(), v.name(), DocumentationKind::BitField)),
        Node::Variant(v) => Some((v.documentation(), v.name(), DocumentationKind::Variant)),
        Node::StorageMember(v) => Some((v.documentation(), v.path(), DocumentationKind::Field)),
        Node::StorageField(v) => Some((v.documentation(), v.name(), DocumentationKind::Field)),
        Node::Reference(_)
        | Node::Scalar(_)
        | Node::Bits(_)
        | Node::Enum(_)
        | Node::Expression(_) => None,
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
        Node::Reference(reference) => B::Domain::reference(reference, ctx, backend, writer),
        Node::Scalar(value) => backend.scalar(view::Scalar(value), ctx, writer),
        Node::StorageField(value) => backend.storage_field(view::StorageField(value), ctx, writer),
        Node::StorageMember(value) => {
            backend.storage_member(view::StorageMember(value), ctx, writer)
        }
        Node::Opaque(value) => backend.opaque(view::Storage(*value), ctx, writer),
        Node::Bits(value) => backend.bits(view::Bits(value, endian), ctx, writer),
        Node::Enum(value) => backend.enumeration(view::Enumeration(value), ctx, writer),
        Node::BitField(value) => {
            backend.bit_field(view::BitField::from_spec(value.spec()), ctx, writer)
        }
        Node::Variant(value) => backend.variant(value, ctx, writer),
        Node::Expression(value) => backend.expression(view::Expression(value), ctx, writer),
    }
}
