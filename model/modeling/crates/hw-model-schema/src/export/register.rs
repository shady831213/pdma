//! Independent register-domain export callbacks over completed semantics.
mod backend;
mod context;
mod documentation;
mod emit;
mod node;
pub use backend::Backend;
pub use context::{EmitContext, LayoutProperty, LayoutPropertyKind, RegisterSite};
pub use node::Node;
/// Register source domain for a target's shared value callbacks.
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
            Some(crate::ast::DefinitionBody::Record(_) | crate::ast::DefinitionBody::Union(_))
        ) {
            Err(crate::analysis::AnalysisError::Domain {
                name: unit.name.clone(),
                domain: "register",
            })
        } else {
            Ok(())
        }
    }
    fn roots(
        schema: &crate::analysis::AnalyzedSchema,
    ) -> impl Iterator<Item = super::value::Visit<B::Context<'_>>> {
        schema
            .units()
            .map(|unit| EmitContext::root(schema, unit, Node::Root(unit)).current())
    }
    fn root(visit: super::value::Visit<B::Context<'_>>, backend: &mut B) -> Result<(), B::Error> {
        backend.root(visit.into_context())
    }
}
