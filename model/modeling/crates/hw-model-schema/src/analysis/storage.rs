use super::{AnalysisError, AnalyzedSchema};
use crate::{
    ResolvedRecord,
    ast::value,
    traversal::{NodeId, record, register},
};
use std::collections::HashMap;
impl AnalyzedSchema {
    pub(super) fn prepare_storage(&mut self) -> Result<(), AnalysisError> {
        let mut layouts = HashMap::new();
        for root in record::definitions(self) {
            collect_record(root.context(), &mut layouts)?;
        }
        for root in register::definitions(self) {
            collect_register(root.context(), &mut layouts)?;
        }
        self.opaque_layouts = layouts;
        let mut unions = HashMap::new();
        for root in record::definitions(self) {
            collect_unions(root.context(), &mut unions);
        }
        self.union_storage = unions;
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

fn collect_unions(
    ctx: &record::Context<'_>,
    storage: &mut HashMap<NodeId, Vec<(usize, crate::FieldSpec)>>,
) {
    use crate::ast::record::Node;
    if ctx
        .node()
        .reference()
        .is_some_and(|r| r.selector_binding().is_some())
        && let Some(field) = ctx.field()
        && let Some(record) = ctx.ancestors().iter().rev().find_map(|node| match node {
            Node::Record(record) => Some(record),
            _ => None,
        })
    {
        let members = record
            .spec()
            .fields()
            .iter()
            .enumerate()
            .filter(|(_, member)| {
                member.offset() >= field.offset()
                    && member.offset().saturating_sub(field.offset()) < field.width()
            })
            .map(|(index, member)| (index, *member))
            .collect();
        storage.insert(ctx.id().clone(), members);
    }
    for child in ctx.children() {
        collect_unions(child.context(), storage);
    }
}
impl AnalyzedSchema {
    /// Physical fields prepared for a tagged-union reference in its declaring record.
    /// Indices retain their original positions in the record's physical storage.
    pub fn union_storage(&self, id: &NodeId) -> Option<&[(usize, crate::FieldSpec)]> {
        self.union_storage.get(id).map(Vec::as_slice)
    }
}
