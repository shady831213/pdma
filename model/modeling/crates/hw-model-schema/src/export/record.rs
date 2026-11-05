//! Independent record-domain export callbacks over completed semantics.
mod backend;
mod emit;
pub use backend::Backend;
/// Record source domain for a target's shared value callbacks.
#[derive(Debug)]
pub struct Domain;
impl<B: Backend> super::Domain<B> for Domain {
    fn emit(
        visit: crate::traversal::Visit<B::Context<'_>>,
        backend: &mut B,
    ) -> Result<(), B::Error> {
        let ctx = visit.into_context();
        emit::emit(ctx.node(), ctx, backend)
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
    fn roots(
        schema: &crate::analysis::AnalyzedSchema,
    ) -> impl Iterator<Item = crate::traversal::Visit<B::Context<'_>>> {
        crate::traversal::record::roots(schema)
    }
    fn root(
        visit: crate::traversal::Visit<B::Context<'_>>,
        backend: &mut B,
    ) -> Result<(), B::Error> {
        backend.root(visit.into_context())
    }
}
