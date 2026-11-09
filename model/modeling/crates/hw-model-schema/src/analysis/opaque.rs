use super::{AnalysisError, AnalyzedSchema};
use crate::{
    ResolvedRecord,
    ast::value,
    traversal::{NodeId, record, register},
};
use std::collections::HashMap;
impl AnalyzedSchema {
    pub(super) fn prepare_opaque(&mut self) -> Result<(), AnalysisError> {
        let mut layouts = HashMap::new();
        for root in record::roots(self) {
            collect_record(root.context(), &mut layouts)?;
        }
        for root in register::roots(self) {
            collect_register(root.context(), &mut layouts)?;
        }
        self.opaque_layouts = layouts;
        Ok(())
    }
    /// Prepared local storage for an opaque containment occurrence.
    pub fn opaque_layout(&self, id: &NodeId) -> Option<&ResolvedRecord> {
        self.opaque_layouts.get(id)
    }
}
fn insert(
    id: &NodeId,
    spec: crate::RecordSpec,
    layouts: &mut HashMap<NodeId, ResolvedRecord>,
) -> Result<(), AnalysisError> {
    let resolved = spec.resolve().map_err(|source| AnalysisError::Record {
        path: spec.name(),
        source,
    })?;
    layouts.insert(id.clone(), resolved);
    Ok(())
}
fn collect_record(
    ctx: &record::Context<'_>,
    layouts: &mut HashMap<NodeId, ResolvedRecord>,
) -> Result<(), AnalysisError> {
    if let crate::ast::record::Node::Value(value::Node::Opaque(spec)) = ctx.node() {
        insert(ctx.id(), *spec, layouts)?;
    }
    for child in ctx.children() {
        collect_record(child.context(), layouts)?;
    }
    Ok(())
}
fn collect_register(
    ctx: &register::Context<'_>,
    layouts: &mut HashMap<NodeId, ResolvedRecord>,
) -> Result<(), AnalysisError> {
    if let register::Node::Value(value::Node::Opaque(spec)) = ctx.node() {
        insert(ctx.id(), *spec, layouts)?;
    }
    for child in ctx.children() {
        collect_register(child.context(), layouts)?;
    }
    Ok(())
}
