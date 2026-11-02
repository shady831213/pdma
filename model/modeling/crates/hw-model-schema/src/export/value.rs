//! Recursive value callbacks shared by independent domain emitters.
mod documentation;
use super::Artifact;
use crate::{
    BitsFieldSpec, EnumDefinition, EnumVariantSpec, RecordSpec,
    ast::{BitsDefinition, LayoutExpr, value::Node},
};
pub use documentation::{
    DocumentationFragment, DocumentationKind, DocumentationOwner, documentation_fragments,
    emit_documentation,
};
/// Read-only bound traversal context implemented by the framework domains.
pub trait Context: Clone {
    /// Domain node type, exposed only for inspection.
    type Node: Copy;
    /// Current semantic node.
    fn node(&self) -> Self::Node;
    /// Current bound visit.
    fn current(&self) -> Visit<Self>;
    /// Bound semantic children.
    fn children(&self) -> std::vec::IntoIter<Visit<Self>>;
}
/// A semantic node bound to its original lexical context.
///
/// Targets cannot manufacture a visit from a context:
/// ```compile_fail
/// use hw_model_schema::export::{record::EmitContext, value::Visit};
/// fn rebind(ctx: EmitContext<'_>) { let _ = Visit::new(ctx); }
/// ```
/// A struct literal also cannot create an independently bound visit:
/// ```compile_fail
/// use hw_model_schema::export::{record::EmitContext, value::Visit};
/// fn forge(ctx: EmitContext<'_>) { let _ = Visit { ctx }; }
/// ```
/// Nor can targets dispatch a raw node paired with a context:
/// ```compile_fail
/// use hw_model_schema::{ast::record::Node, export::{record, value::Backend}};
/// fn rebind<B: record::Backend>(b: &mut B, node: Node<'_>, ctx: record::EmitContext<'_>) {
///     b.emit(node, ctx);
/// }
/// ```
#[derive(Debug, Clone)]
pub struct Visit<S> {
    ctx: S,
}
impl<S: Context> Visit<S> {
    pub(crate) const fn new(ctx: S) -> Self {
        Self { ctx }
    }
    /// Inspects the semantic node without granting rebinding capability.
    pub fn node(&self) -> S::Node {
        self.ctx.node()
    }
    /// Inspects the immutable lexical context.
    pub const fn context(&self) -> &S {
        &self.ctx
    }
    pub(crate) fn into_context(self) -> S {
        self.ctx
    }
}
/// Target handling of values in one independently chosen source domain.
pub trait Backend: Sized {
    /// Independently selected source domain.
    type Domain: super::Domain<Self>;
    /// Target-owned failure.
    type Error;
    /// Domain-specific immutable recursion context.
    type Context<'a>: Context;
    /// Dispatches one framework-bound visit.
    fn emit(&mut self, visit: Visit<Self::Context<'_>>) -> Result<(), Self::Error> {
        <Self::Domain as super::Domain<Self>>::emit(visit, self)
    }
    /// Dispatches bound visits in target-selected order, stopping at the first error.
    fn emit_all<'a, I>(&mut self, visits: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Visit<Self::Context<'a>>>,
    {
        for visit in visits {
            self.emit(visit)?;
        }
        Ok(())
    }
    /// Handles reusable or inline bits.
    fn bits<'a>(
        &mut self,
        _value: &'a BitsDefinition,
        ctx: Self::Context<'a>,
    ) -> Result<(), Self::Error> {
        self.emit_all(ctx.children())
    }
    /// Handles an enumeration and its variants.
    fn enumeration(
        &mut self,
        _value: EnumDefinition,
        ctx: Self::Context<'_>,
    ) -> Result<(), Self::Error> {
        self.emit_all(ctx.children())
    }
    /// Handles one bit field.
    fn bit_field<'a>(
        &mut self,
        _value: &'a BitsFieldSpec,
        ctx: Self::Context<'a>,
    ) -> Result<(), Self::Error> {
        self.emit_all(ctx.children())
    }
    /// Handles one enum alternative.
    fn variant(&mut self, _: &EnumVariantSpec, _: Self::Context<'_>) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Handles a mathematical expression and recursively visits operands by default.
    fn expression<'a>(
        &mut self,
        _value: &'a LayoutExpr,
        ctx: Self::Context<'a>,
    ) -> Result<(), Self::Error> {
        self.emit_all(ctx.children())
    }
    /// Handles an opaque byte envelope shared by both source domains.
    fn opaque(&mut self, _: &RecordSpec, _: Self::Context<'_>) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Handles a complete short description before its semantic owner callback.
    fn short_description(
        &mut self,
        _: &str,
        _: DocumentationOwner<'_>,
        _: Self::Context<'_>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Handles a complete detailed description before its semantic owner callback.
    fn description(
        &mut self,
        _: &str,
        _: DocumentationOwner<'_>,
        _: Self::Context<'_>,
    ) -> Result<(), Self::Error> {
        Ok(())
    }
    /// Returns artifacts after every selected definition succeeded.
    fn finish(self) -> Result<Vec<Artifact>, Self::Error>;
}
/// Dispatches a shared semantic value node.
///
/// # Errors
/// Propagates the target error.
pub(super) fn emit<'a, B: Backend>(
    node: Node<'a>,
    ctx: B::Context<'a>,
    backend: &mut B,
) -> Result<(), B::Error> {
    let documentation = match node {
        Node::Opaque(v) => Some((v.documentation(), v.name(), DocumentationKind::Opaque)),
        Node::BitField(v) => Some((v.documentation(), v.name(), DocumentationKind::BitField)),
        Node::Variant(v) => Some((v.documentation(), v.name(), DocumentationKind::Variant)),
        Node::Bits(_) | Node::Enum(_) | Node::Expression(_) => None,
    };
    if let Some((text, name, kind)) = documentation {
        emit_documentation(text, DocumentationOwner { name, kind }, &ctx, backend)?;
    }
    match node {
        Node::Opaque(value) => backend.opaque(value, ctx),
        Node::Bits(value) => backend.bits(value, ctx),
        Node::Enum(value) => backend.enumeration(value, ctx),
        Node::BitField(value) => backend.bit_field(value, ctx),
        Node::Variant(value) => backend.variant(value, ctx),
        Node::Expression(value) => backend.expression(value, ctx),
    }
}
