//! Independent record-domain export callbacks over completed semantics.
mod backend;
mod context;
mod emit;
mod storage;
pub use backend::Backend;
pub use context::{Context, entries};
/// Record source domain for a target's shared value callbacks.
#[derive(Debug)]
pub struct Domain;
impl<B: Backend> super::context::sealed::Domain<B> for Domain {}
impl<B: Backend> super::Domain<B> for Domain {
    fn emit(
        visit: super::Visit<B::Context<'_>>,
        backend: &mut B,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), B::Error> {
        let ctx = visit.into_context();
        emit::emit(ctx.0.node(), ctx, backend, writer)
    }
    fn reference<'a>(
        reference: &'a crate::ast::Reference,
        ctx: B::Context<'a>,
        backend: &mut B,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), B::Error> {
        emit::emit(
            crate::ast::record::Node::Reference(reference),
            ctx,
            backend,
            writer,
        )
    }
    fn validate(unit: &crate::analysis::Unit) -> Result<(), crate::analysis::AnalysisError> {
        if matches!(
            unit.declaration,
            Some(
                crate::ast::DefinitionBody::Register(_)
                    | crate::ast::DefinitionBody::RegisterMap(_)
            )
        ) || matches!(unit.body, crate::analysis::Layout::Map(_))
        {
            Err(crate::analysis::AnalysisError::Domain {
                name: unit.name.clone(),
                domain: "record",
            })
        } else {
            Ok(())
        }
    }
    fn entries(
        schema: &crate::analysis::AnalyzedSchema,
    ) -> impl Iterator<Item = super::Visit<B::Context<'_>>> {
        entries(schema)
    }
    fn entry(
        visit: super::Visit<B::Context<'_>>,
        backend: &mut B,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), B::Error> {
        backend.entry(visit.into_context(), writer)
    }
}
