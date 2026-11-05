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
    unions: HashMap<NodeId, Vec<UnionMember>>,
    opaque: HashMap<NodeId, Vec<(String, usize)>>,
}
#[derive(Debug)]
pub(super) struct UnionMember {
    pub(super) path: String,
    pub(super) field: crate::FieldSpec,
    pub(super) offset: usize,
}
impl CRecordAnalysis {
    /// Absolute byte offset of one source field occurrence.
    pub fn offset(&self, id: &NodeId) -> Option<usize> {
        self.offsets.get(id).copied()
    }
    pub(super) fn union_storage(
        &self,
        ctx: &Context<'_>,
    ) -> Result<&[UnionMember], CHeaderExportError> {
        self.unions
            .get(ctx.id())
            .map(Vec::as_slice)
            .ok_or(CHeaderExportError::InvalidVisit("missing C union analysis"))
    }
    pub(super) fn opaque_storage(
        &self,
        ctx: &Context<'_>,
    ) -> Result<&[(String, usize)], CHeaderExportError> {
        self.opaque
            .get(ctx.id())
            .map(Vec::as_slice)
            .ok_or(CHeaderExportError::InvalidVisit(
                "missing C opaque analysis",
            ))
    }
    pub(super) fn at(&self, ctx: &Context<'_>) -> Result<usize, CHeaderExportError> {
        let mut id = Some(ctx.id().clone());
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
            unions: HashMap::new(),
            opaque: HashMap::new(),
        };
        for root in record::roots(schema) {
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
        let base = if ctx.field().is_some() {
            output.at(ctx)?
        } else {
            0
        };
        let schema = opaque_schema(ctx)?;
        let mut members = Vec::new();
        for member in schema.members() {
            let offset = base
                .checked_add(member.absolute_offset())
                .ok_or(CHeaderExportError::InvalidVisit("C opaque offset overflow"))?;
            members.push((opaque_path(ctx, schema, member.path()), offset));
        }
        output.opaque.insert(ctx.id().clone(), members);
    }
    if matches!(ctx.node(), Node::Union(_)) {
        let base = ctx
            .ancestors()
            .iter()
            .filter_map(|node| match node {
                Node::Field(f) => Some(f.offset()),
                _ => None,
            })
            .try_fold(0_usize, usize::checked_add)
            .ok_or(CHeaderExportError::InvalidVisit("C union offset overflow"))?;
        let local = ctx.field().map_or(0, crate::ast::Field::offset);
        let mut members = Vec::new();
        for (path, field) in storage_words(ctx) {
            let offset = base
                .checked_sub(local)
                .and_then(|base| base.checked_add(field.offset()))
                .ok_or(CHeaderExportError::InvalidVisit("C union offset overflow"))?;
            members.push(UnionMember {
                path,
                field,
                offset,
            });
        }
        output.unions.insert(ctx.id().clone(), members);
        return Ok(());
    }
    for child in ctx.children() {
        collect(child.context(), output)?;
    }
    if let Some(referent) = ctx.referent() {
        collect(referent.context(), output)?;
    }
    Ok(())
}
pub(super) fn path(ctx: &Context<'_>) -> String {
    ctx.ancestors()
        .iter()
        .copied()
        .chain([ctx.node()])
        .filter_map(|node| match node {
            Node::Field(field) => Some(field.name()),
            Node::Case(case) => Some(case.name()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(".")
}

fn storage_words(ctx: &Context<'_>) -> Vec<(String, crate::FieldSpec)> {
    let Node::Union(union) = ctx.node() else {
        return Vec::new();
    };
    if union.selector().is_none() {
        return Vec::new();
    }
    let Some(field) = ctx.field() else {
        return Vec::new();
    };
    let Some(record) = ctx.ancestors().iter().rev().find_map(|node| match node {
        Node::Record(r) => Some(r),
        _ => None,
    }) else {
        return Vec::new();
    };
    let path = path(ctx);
    let parent = path.rsplit_once('.').map_or("", |(parent, _)| parent);
    record
        .spec()
        .fields()
        .iter()
        .filter(|member| {
            member.offset() >= field.offset()
                && member.offset().saturating_sub(field.offset()) < field.width()
        })
        .map(|member| {
            (
                if parent.is_empty() {
                    member.name().into()
                } else {
                    format!("{parent}.{}", member.name())
                },
                *member,
            )
        })
        .collect()
}

pub(super) fn opaque_schema<'a>(
    ctx: &Context<'a>,
) -> Result<&'a crate::ResolvedRecord, CHeaderExportError> {
    let unit = ctx
        .ancestors()
        .iter()
        .rev()
        .find_map(|node| node.reference())
        .and_then(|reference| ctx.resolve(reference))
        .unwrap_or_else(|| ctx.unit());
    match &unit.body {
        crate::analysis::Layout::Record(schema) => Ok(schema),
        _ => Err(CHeaderExportError::InvalidVisit("opaque record layout")),
    }
}
pub(super) fn opaque_path(
    ctx: &Context<'_>,
    schema: &crate::ResolvedRecord,
    member: &str,
) -> String {
    let relative = super::naming::relative_path(schema.root(), member);
    let parent = path(ctx);
    if parent.is_empty() {
        relative.into()
    } else {
        format!("{parent}.{relative}")
    }
}
