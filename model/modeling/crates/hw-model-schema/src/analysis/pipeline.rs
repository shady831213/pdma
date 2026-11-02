//! The default pass sequence is ordinary Rust composition, with no scheduler.
use super::{AnalysisError, AnalyzedSchema, Bindings};
use crate::ast::{SchemaGraph, SchemaSet};
impl AnalyzedSchema {
    /// Resolves, validates, lays out and simplifies a selected schema.
    pub fn analyze(set: SchemaSet, bindings: &Bindings) -> Result<Self, AnalysisError> {
        let graph = super::resolve::run(set)?;
        Self::from_graph(&graph, bindings)
    }
    /// Continues analysis from an already resolved definition graph.
    pub fn from_graph(graph: &SchemaGraph, bindings: &Bindings) -> Result<Self, AnalysisError> {
        Self::finish(prepare_graph(graph, bindings)?)
    }
    pub(super) fn finish(mut units: Vec<super::Unit>) -> Result<Self, AnalysisError> {
        for unit in &units {
            super::validate::node(unit.node()?)?;
        }
        super::simplify::run(&mut units)?;
        Ok(Self { units })
    }
}
pub(super) fn prepare_graph(
    graph: &SchemaGraph,
    bindings: &Bindings,
) -> Result<Vec<super::Unit>, AnalysisError> {
    super::resolve::bindings(graph, bindings)?;
    super::validate::run(graph)?;
    super::layout::run(graph, bindings)
}
