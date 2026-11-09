//! Independent register-domain export callbacks over completed semantics.
mod backend;
mod context;
mod documentation;
mod emit;
mod placement;
mod reference;
pub use super::view::Bank;
pub use super::view::LayoutProperty;
pub use crate::traversal::register::BankProperty;
use crate::traversal::register::Node;
pub use backend::Backend;
pub use context::{Context, roots};
/// Register source domain for a target's shared value callbacks.
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
    ) -> impl Iterator<Item = super::Visit<B::Context<'_>>> {
        roots(schema)
    }
    fn root(
        visit: super::Visit<B::Context<'_>>,
        backend: &mut B,
        writer: &mut dyn std::io::Write,
    ) -> Result<(), B::Error> {
        backend.root(visit.into_context(), writer)
    }
}
