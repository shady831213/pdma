//! Independent register-domain export callbacks over completed semantics.
mod backend;
mod documentation;
mod emit;
use crate::traversal::register::{Bank, BankProperty, Context, LayoutProperty, Node};
pub use backend::Backend;
/// Register source domain for a target's shared value callbacks.
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
        if crate::traversal::register::supports(unit) {
            Ok(())
        } else {
            Err(crate::analysis::AnalysisError::Domain {
                name: unit.name.clone(),
                domain: "register",
            })
        }
    }
    fn roots(
        schema: &crate::analysis::AnalyzedSchema,
    ) -> impl Iterator<Item = crate::traversal::Visit<B::Context<'_>>> {
        crate::traversal::register::roots(schema)
    }
    fn root(
        visit: crate::traversal::Visit<B::Context<'_>>,
        backend: &mut B,
    ) -> Result<(), B::Error> {
        backend.root(visit.into_context())
    }
}
