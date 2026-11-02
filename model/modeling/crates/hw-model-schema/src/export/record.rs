//! Independent record-domain export callbacks over completed semantics.
mod backend;
mod context;
mod emit;
pub use backend::Backend;
pub use context::EmitContext;
/// Record source domain for a target's shared value callbacks.
#[derive(Debug)]
pub struct Domain;
impl<B: Backend> super::Domain<B> for Domain {
    fn emit(visit: super::value::Visit<B::Context<'_>>, backend: &mut B) -> Result<(), B::Error> {
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
    ) -> impl Iterator<Item = super::value::Visit<B::Context<'_>>> {
        schema.units().filter_map(|unit| {
            let node = match unit.declaration.as_ref()?.node() {
                crate::ast::Node::Record(v) => v,
                crate::ast::Node::Value(v) => crate::ast::record::Node::Value(v),
                crate::ast::Node::Register(_) => return None,
            };
            Some(EmitContext::root(schema, unit, node).current())
        })
    }
    fn root(visit: super::value::Visit<B::Context<'_>>, backend: &mut B) -> Result<(), B::Error> {
        backend.root(visit.into_context())
    }
}
