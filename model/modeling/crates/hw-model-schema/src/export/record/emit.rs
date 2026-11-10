use super::Backend;
use super::Context;
use crate::ast::record::Node;
use crate::export::view;
/// Dispatches a record node with recursion-local context.
///
/// # Errors
/// Propagates the target error.
pub(super) fn emit<'a, B: Backend>(
    node: Node<'a>,
    ctx: Context<'a>,
    backend: &mut B,
    writer: &mut dyn std::io::Write,
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
        Node::Value(crate::ast::value::Node::Bits(_) | crate::ast::value::Node::Enum(_))
            if ctx.is_entry() =>
        {
            Some((
                ctx.0.unit().documentation,
                ctx.0.unit().name.as_str(),
                super::super::value::DocumentationKind::Definition,
            ))
        }
        Node::Case(_)
        | Node::Reference(_)
        | Node::Attribute(_)
        | Node::Constant(_)
        | Node::Value(_) => None,
    };
    if let Some((text, name, kind)) = documentation {
        super::super::value::emit_documentation(
            text,
            super::super::value::DocumentationOwner { name, kind },
            &ctx,
            backend,
            writer,
        )?;
    }
    match node {
        Node::Record(v) => backend.record(view::Record(v), ctx, writer),
        Node::Union(v) => backend.union(view::Union(v), ctx, writer),
        Node::Field(v) => backend.field(view::Field(v), ctx, writer),
        Node::Case(v) => backend.case(view::Case(v), ctx, writer),
        Node::Reference(v) => ctx.0.referent().map_or_else(
            || Ok(()),
            |target| {
                backend.reference(
                    view::Reference(v),
                    crate::export::Visit::new(Context(target.into_context())),
                    ctx,
                    writer,
                )
            },
        ),
        Node::Attribute(_) => Ok(()),
        Node::Constant(v) => backend.constant(v, ctx, writer),
        Node::Value(v) => {
            let endian = ctx
                .field()
                .map_or(crate::Endian::Little, view::Field::endian);
            // Byte order belongs to the field occurrence, not its reusable enum type.
            // Physical storage projections already carry their own encoding.
            let v = match v {
                crate::ast::value::Node::Enum(crate::ast::value::Enumeration::Scalar(value))
                    if ctx.parent_kind() == Some(crate::export::Kind::Reference) =>
                {
                    crate::ast::value::Node::Enum(crate::ast::value::Enumeration::Scalar(
                        value.with_endian(endian),
                    ))
                }
                other => other,
            };
            super::super::value::emit(v, endian, ctx, backend, writer)
        }
    }
}
