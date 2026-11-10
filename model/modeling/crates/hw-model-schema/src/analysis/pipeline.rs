//! The default pass sequence is ordinary Rust composition, with no scheduler.
use super::{
    AnalysisError, AnalyzedSchema, Bindings, LayoutSchema, Pass, Resolve, Simplify, Validate,
};
use crate::ast::{SchemaGraph, SchemaSet};
impl AnalyzedSchema {
    /// Resolves, validates, lays out and simplifies a selected schema.
    pub fn analyze(set: SchemaSet, bindings: &Bindings) -> Result<Self, AnalysisError> {
        let graph = Resolve.run(set)?;
        Self::from_graph(&graph, bindings)
    }
    /// Continues analysis from an already resolved definition graph.
    pub fn from_graph(graph: &SchemaGraph, bindings: &Bindings) -> Result<Self, AnalysisError> {
        Simplify.run(LayoutSchema.run(Validate::new(bindings).run(graph)?)?)
    }
    pub(super) fn finish(units: Vec<super::Unit>) -> Result<Self, AnalysisError> {
        for unit in &units {
            super::validate::node(unit.node()?)?;
        }
        Self::simplify_units(units)
    }
    pub(super) fn simplify_units(mut units: Vec<super::Unit>) -> Result<Self, AnalysisError> {
        super::simplify::run(&mut units)?;
        let mut schema = Self {
            units,
            opaque_layouts: std::collections::HashMap::new(),
            union_storage: std::collections::HashMap::new(),
        };
        schema.prepare_storage()?;
        Ok(schema)
    }
}
pub(super) fn prepare_graph(
    graph: &SchemaGraph,
    bindings: &Bindings,
) -> Result<Vec<super::Unit>, AnalysisError> {
    Ok(LayoutSchema
        .run(Validate::new(bindings).run(graph)?)?
        .into_units())
}
