use crate::traversal::record::Context;
mod accessors;
mod opaque;

use super::analysis;
use super::{CHeaderExportError, CRecordBackend, fragment::Fragment, naming};
use crate::ast::{Field, FieldType, RecordDefinition, UnionDefinition, record::Node};
use crate::export::value::Backend as _;
use crate::export::{Artifact, record::Backend};
use crate::{EnumDefinition, ResolvedMemberKind};
type CResult<T = ()> = Result<T, CHeaderExportError>;
impl Backend for CRecordBackend<'_> {
    fn root(&mut self, ctx: Context<'_>) -> CResult {
        (self.logical_name, self.prefix) = self.names.unit(ctx.unit())?;
        self.output = Fragment::default();
        self.emit(ctx.current())?;
        self.artifacts.push(Artifact {
            logical_name: self.logical_name.clone(),
            media_type: "text/x-c; charset=utf-8".into(),
            contents: std::mem::take(&mut self.output.text).into_bytes(),
        });
        Ok(())
    }
    fn record(&mut self, value: &RecordDefinition, ctx: Context<'_>) -> CResult {
        if ctx.ancestors().is_empty() {
            self.header(value.spec())?;
        }
        self.emit_all(ctx.children())
    }
    fn field(&mut self, field: &Field, ctx: Context<'_>) -> CResult {
        let path = analysis::path(&ctx);
        let offset = self.analysis.at(&ctx)?;
        match field.ty() {
            FieldType::Scalar { definition, .. } | FieldType::Enum(definition) => {
                self.field_intro(field)?;
                self.emit_all(ctx.children())?;
                self.scalar(&path, offset, *definition)
            }
            FieldType::Bits(_) => {
                self.field_intro(field)?;
                self.emit_all(ctx.children())
            }
            FieldType::Reference(_) => {
                let visit = ctx
                    .referent()
                    .ok_or(CHeaderExportError::InvalidVisit("record referent"))?;
                if matches!(visit.node(), Node::Value(crate::ast::value::Node::Bits(_))) {
                    self.field_intro(field)?;
                } else {
                    let documentation = match visit.node() {
                        Node::Record(record) => record.spec().documentation(),
                        Node::Union(union) => union.storage().documentation(),
                        Node::Value(crate::ast::value::Node::Opaque(spec)) => spec.documentation(),
                        _ => crate::Documentation::EMPTY,
                    };
                    self.placement(field, &ctx, documentation)?;
                }
                self.emit(visit)
            }
            FieldType::Union(_) => self.emit_all(ctx.children()),
        }
    }
    fn union(&mut self, value: &UnionDefinition, ctx: Context<'_>) -> CResult {
        if ctx.ancestors().is_empty() {
            return self.header(value.storage());
        }
        let analysis = self.analysis;
        let storage = analysis.union_storage(&ctx)?;
        if storage.is_empty()
            && let Some(field) = ctx.field()
            && matches!(field.ty(), FieldType::Union(_))
        {
            self.placement(field, &ctx, value.storage().documentation())?;
        }
        for member in storage {
            let (path, field) = (&member.path, &member.field);
            self.output.text.push('\n');
            self.output.documentation("", field.documentation())?;
            self.scalar(path, member.offset, field.definition())?;
        }
        Ok(())
    }
}

impl CRecordBackend<'_> {
    fn placement(
        &mut self,
        field: &Field,
        ctx: &Context<'_>,
        documentation: crate::Documentation,
    ) -> CResult {
        let path = analysis::path(ctx);
        let root = ctx
            .ancestors()
            .iter()
            .find_map(|node| match node {
                Node::Record(record) => Some(record.spec().name()),
                _ => None,
            })
            .ok_or(CHeaderExportError::InvalidVisit("record reference parent"))?;
        self.output.documentation(
            &format!("{root}.{path}"),
            field.documentation().with_fallback(documentation),
        )?;
        self.output.constants::<64>(
            &format!("{}_RECORD_{}", self.prefix, naming::c_identifier(&path)),
            &[("OFFSET", self.analysis.at(ctx)?), ("SIZE", field.width())],
        )
    }

    fn header(&mut self, spec: crate::RecordSpec) -> CResult {
        self.output.documentation("", spec.documentation())?;
        self.output.constants::<64>(
            &self.prefix,
            &[("SIZE", spec.size()), ("ALIGNMENT", spec.alignment())],
        )?;
        self.output
            .constants::<32>(&self.prefix, &[("ENDIAN_LITTLE", 0), ("ENDIAN_BIG", 1)])
    }
    fn field_intro(&mut self, field: &Field) -> CResult {
        self.output.text.push('\n');
        self.output.documentation("", field.documentation())
    }
    pub(super) fn scalar(
        &mut self,
        path: &str,
        offset: usize,
        definition: crate::FieldDefinition,
    ) -> CResult {
        let name = naming::c_identifier(path);
        let metadata = format!("{}_{}", self.prefix, name);
        self.output
            .constants::<64>(&metadata, &[("OFFSET", offset)])?;
        accessors::scalar_accessors(
            &mut self.output,
            accessors::FieldNames {
                prefix: &self.prefix,
                field: &name,
                metadata: &metadata,
            },
            definition,
        )
    }

    pub(super) fn enum_constants(
        &mut self,
        prefix: &str,
        enumeration: EnumDefinition,
    ) -> Result<(), CHeaderExportError> {
        for variant in enumeration.variants() {
            self.output
                .documentation(variant.name(), variant.documentation())?;
            self.output.unsigned::<64>(
                &format!("{prefix}_{}", naming::c_identifier(variant.name())),
                variant.value(),
            )?;
        }
        Ok(())
    }
}
