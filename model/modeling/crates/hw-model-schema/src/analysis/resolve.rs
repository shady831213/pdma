//! Definition identity and root-binding resolution.
use super::{AnalysisError, Bindings};
use crate::ast::{DefinitionBody, SchemaGraph, SchemaSet};
/// Resolves source references and rejects recursive or ambiguous definitions.
pub fn run(set: SchemaSet) -> Result<SchemaGraph, AnalysisError> {
    set.resolve().map_err(AnalysisError::Dependencies)
}
pub(super) fn bindings(graph: &SchemaGraph, bindings: &Bindings) -> Result<(), AnalysisError> {
    for id in bindings.values.keys() {
        if !graph.roots().contains(id)
            || !graph
                .get(*id)
                .is_some_and(|d| matches!(d.body(), DefinitionBody::RegisterMap(_)))
        {
            return Err(AnalysisError::Binding(*id));
        }
        if graph
            .definitions()
            .any(|d| d.references().iter().any(|r| r.target().id() == *id))
        {
            return Err(AnalysisError::ReferencedBinding(*id));
        }
    }
    Ok(())
}
