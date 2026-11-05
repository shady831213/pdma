use super::Backend;
use crate::ast::record::{Attribute, Node};
use crate::traversal::record::Context;
/// Dispatches a record node with recursion-local context.
///
/// # Errors
/// Propagates the target error.
pub(super) fn emit<'a, B: Backend>(
    node: Node<'a>,
    ctx: Context<'a>,
    backend: &mut B,
) -> Result<(), B::Error> {
    let documentation = match node {
        Node::Record(v) => Some((
            v.spec().documentation(),
            v.spec().name(),
            super::super::value::DocumentationKind::Record,
        )),
        Node::Union(v) => Some((
            v.storage().documentation(),
            v.storage().name(),
            super::super::value::DocumentationKind::Union,
        )),
        Node::Field(v) => Some((
            v.documentation(),
            v.name(),
            super::super::value::DocumentationKind::Field,
        )),
        Node::Case(v) => match v.body() {
            crate::ast::CaseBody::Inline(r) => Some((
                r.spec().documentation(),
                v.name(),
                super::super::value::DocumentationKind::Case,
            )),
            crate::ast::CaseBody::Reference(_) => None,
        },
        Node::Value(crate::ast::value::Node::Bits(_)) if ctx.field().is_none() => Some((
            ctx.unit().documentation,
            ctx.unit().name.as_str(),
            super::super::value::DocumentationKind::Definition,
        )),
        Node::Attribute(_) | Node::Constant(_) | Node::Value(_) => None,
    };
    if let Some((text, name, kind)) = documentation {
        super::super::value::emit_documentation(
            text,
            super::super::value::DocumentationOwner { name, kind },
            &ctx,
            backend,
        )?;
    }
    match node {
        Node::Record(v) => backend.record(v, ctx),
        Node::Union(v) => backend.union(v, ctx),
        Node::Field(v) => backend.field(v, ctx),
        Node::Case(v) => backend.case(v, ctx),
        Node::Attribute(v) => match (v, matches!(ctx.ancestors().last(), Some(Node::Field(_)))) {
            (Attribute::Size(v), false) => backend.record_size(v, ctx),
            (Attribute::Size(v), true) => backend.field_size(v, ctx),
            (Attribute::Alignment(v), _) => backend.record_alignment(v, ctx),
            (Attribute::Offset(v), _) => backend.field_offset(v, ctx),
            (Attribute::Endian(v), false) => backend.record_endian(v, ctx),
            (Attribute::Endian(v), true) => backend.field_endian(v, ctx),
        },
        Node::Constant(v) => backend.constant(v, ctx),
        Node::Value(v) => super::super::value::emit(v, ctx, backend),
    }
}
