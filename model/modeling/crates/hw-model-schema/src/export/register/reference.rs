use super::{Backend, Context};
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
        kind: context::entry_kind(unit),
        access,
    };
    let Some(target) = ctx.0.referent() else {
        return Ok(());
    };
    let target = Context(target.into_context());
    backend.reference(
        view::Reference(reference),
        metadata,
        Visit::new(target),
        ctx,
        writer,
    )
}
