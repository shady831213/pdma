use super::CHeaderExportError;
use crate::{
    analysis::{AnalyzedSchema, Pass},
    ast::record::Node,
    traversal::{
        NodeId,
        record::{self, Context},
    },
};
use std::collections::HashMap;

/// Optional C preparation of absolute byte offsets for source occurrences.
#[derive(Debug, Clone, Copy)]
pub struct CRecordAnalysisPass;

/// Immutable C-specific results keyed by structural source occurrence.
#[derive(Debug)]
pub struct CRecordAnalysis {
    offsets: HashMap<NodeId, usize>,
    collisions: HashMap<NodeId, crate::export::SymbolCollision<(usize, String)>>,
}
impl CRecordAnalysis {
    pub(super) fn check_names(&self, id: &NodeId) -> Result<(), CHeaderExportError> {
        self.collisions.get(id).map_or(Ok(()), |collision| {
            Err(super::naming::collision(collision.clone()))
        })
    }
    /// Absolute byte offset of one source field occurrence.
    pub fn offset(&self, id: &NodeId) -> Option<usize> {
        self.offsets.get(id).copied()
    }
    pub(super) fn at(&self, source: &NodeId) -> Result<usize, CHeaderExportError> {
        let mut id = Some(source.clone());
        while let Some(current) = id {
            if let Some(offset) = self.offset(&current) {
                return Ok(offset);
            }
            id = current.parent();
        }
        Err(CHeaderExportError::InvalidVisit(
            "missing C occurrence analysis",
        ))
    }
}
impl<'a> Pass<(&'a AnalyzedSchema, ())> for CRecordAnalysisPass {
    type Output = (&'a AnalyzedSchema, CRecordAnalysis);
    type Error = CHeaderExportError;
    fn run(self, (schema, ()): (&'a AnalyzedSchema, ())) -> Result<Self::Output, Self::Error> {
        let mut output = CRecordAnalysis {
            offsets: HashMap::new(),
            collisions: HashMap::new(),
        };
        for root in record::definitions(schema) {
            collect(root.context(), &mut output)?;
        }
        Ok((schema, output))
    }
}
fn collect(ctx: &Context<'_>, output: &mut CRecordAnalysis) -> Result<(), CHeaderExportError> {
    if matches!(ctx.node(), Node::Field(_)) {
        let offset = ctx
            .ancestors()
            .iter()
            .copied()
            .chain([ctx.node()])
            .try_fold(0_usize, |base, node| match node {
                Node::Field(field) => base.checked_add(field.offset()),
                _ => Some(base),
            })
            .ok_or(CHeaderExportError::InvalidVisit(
                "C occurrence offset overflow",
            ))?;
        output.offsets.insert(ctx.id().clone(), offset);
    }
    if matches!(ctx.node(), Node::Value(crate::ast::value::Node::Opaque(_))) {
        let schema = opaque_schema(ctx)?;
        if let Some(collision) = record_collision(schema) {
            output.collisions.insert(ctx.id().clone(), collision);
        }
    }
    if matches!(ctx.node(), Node::Union(_)) {
        return Ok(());
    }
    if let Some(target) = ctx.referent() {
        collect(target.context(), output)?;
    }
    for child in ctx.children() {
        collect(child.context(), output)?;
    }
    Ok(())
}
fn opaque_schema<'a>(ctx: &Context<'a>) -> Result<&'a crate::ResolvedRecord, CHeaderExportError> {
    ctx.opaque_layout()
        .ok_or(CHeaderExportError::InvalidVisit("opaque record layout"))
}

fn record_collision(
    schema: &crate::ResolvedRecord,
) -> Option<crate::export::SymbolCollision<(usize, String)>> {
    let mut symbols = crate::export::SymbolScope::new();
    schema
        .scalar_members()
        .enumerate()
        .find_map(|(index, (member, _))| {
            let path = member
                .path()
                .strip_prefix(schema.root())
                .and_then(|relative| relative.strip_prefix('.'))
                .unwrap_or_else(|| member.path());
            let name = super::naming::c_identifier(path);
            symbols
                .insert(
                    super::naming::significant_prefix(&name),
                    (index, member.path().to_owned()),
                )
                .err()
        })
}
