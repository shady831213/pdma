use super::NodeId;
use crate::{
    analysis::{AnalyzedSchema, Unit},
    ast::{Definition, DefinitionBody, DefinitionId, SchemaGraph},
};
mod sealed {
    pub trait Sealed {}
    impl Sealed for crate::analysis::AnalyzedSchema {}
    impl Sealed for crate::ast::SchemaGraph {}
}
/// Immutable source provider shared by early passes and completed traversal.
///
/// The framework implements this for resolved source graphs and analyzed schemas.
/// Source traversal does not imply validation or manufacture completed layout data.
pub trait Schema: sealed::Sealed {
    /// Actual root data at this stage: a source definition or an analyzed unit.
    type Root;
    /// Reads the root's imported or named identity.
    fn root_id(root: &Self::Root) -> NodeId;
    /// Looks up an imported reference at the frontend identity boundary.
    fn definition(&self, id: DefinitionId) -> Option<&Self::Root>;
    /// Borrows original source declarations; permission projections have none.
    fn declaration(root: &Self::Root) -> Option<&DefinitionBody>;
}
impl Schema for AnalyzedSchema {
    type Root = Unit;
    fn root_id(root: &Unit) -> NodeId {
        root.id.clone()
    }
    fn definition(&self, id: DefinitionId) -> Option<&Unit> {
        self.definition(id)
    }
    fn declaration(root: &Unit) -> Option<&DefinitionBody> {
        root.declaration.as_ref()
    }
}
impl Schema for SchemaGraph {
    type Root = Definition;
    fn root_id(root: &Definition) -> NodeId {
        NodeId::definition(root.id())
    }
    fn definition(&self, id: DefinitionId) -> Option<&Definition> {
        self.get(id)
    }
    fn declaration(root: &Definition) -> Option<&DefinitionBody> {
        Some(root.body())
    }
}
