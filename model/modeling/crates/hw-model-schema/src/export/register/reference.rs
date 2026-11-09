use super::{Backend, Context, Node};
use crate::export::{Visit, context, view};

pub(super) fn emit<'a, B: Backend>(
    reference: &'a crate::ast::Reference,
    ctx: Context<'a>,
    backend: &mut B,
    writer: &mut dyn std::io::Write,
) -> Result<(), B::Error> {
    let Some(unit) = ctx.0.resolve(reference) else {
        return Ok(());
    };
    let access = match unit.declaration.as_ref() {
        Some(crate::ast::DefinitionBody::Register(value)) => Some(value.access()),
        _ => None,
    };
    let metadata = view::Target {
        id: &unit.id,
        name: &unit.name,
        kind: context::root_kind(unit),
        access,
    };
    let node = match unit.declaration.as_ref() {
        Some(crate::ast::DefinitionBody::Bits(bits)) => {
            Node::Value(crate::ast::value::Node::Bits(bits))
        }
        Some(crate::ast::DefinitionBody::Register(register)) => Node::Register(register),
        Some(crate::ast::DefinitionBody::Opaque(spec)) => {
            Node::Value(crate::ast::value::Node::Opaque(spec))
        }
        Some(crate::ast::DefinitionBody::RegisterMap(_)) => Node::Root(unit),
        Some(crate::ast::DefinitionBody::Record(_) | crate::ast::DefinitionBody::Union(_))
        | None => return Ok(()),
    };
    let target = if matches!(unit.declaration, Some(crate::ast::DefinitionBody::Bits(_))) {
        Context(ctx.0.descendant(node, crate::traversal::Edge::Referent))
    } else {
        Context(crate::traversal::register::Context::root(
            ctx.0.analysis(),
            unit,
            Node::Root(unit),
        ))
    };
    backend.reference(
        view::Reference(reference),
        metadata,
        Visit::new(target),
        ctx,
        writer,
    )
}
