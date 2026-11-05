use crate::export::backends::c_header::fragment::Fragment;
use crate::traversal::record::Context;
use std::io::Write;
mod accessors;
mod opaque;

use super::analysis;
use super::{CHeaderExportError, CRecordBackend, naming};
use crate::ast::{Field, FieldType, RecordDefinition, UnionDefinition, record::Node};
use crate::export::record::Backend;
use crate::export::value::Backend as _;
use crate::{EnumDefinition, ResolvedMemberKind};
type CResult<T = ()> = Result<T, CHeaderExportError>;
impl Backend for CRecordBackend<'_> {
    fn root(&mut self, ctx: Context<'_>, writer: &mut dyn Write) -> CResult {
        self.prefix = self.names.unit_name(ctx.unit()).1;
        self.output = Fragment::default();
        self.emit(ctx.current(), writer)
    }
    fn record(
        &mut self,
        value: &RecordDefinition,
        ctx: Context<'_>,
        writer: &mut dyn Write,
    ) -> CResult {
        if ctx.ancestors().is_empty() {
            self.header(value.spec(), writer)?;
        }
        self.emit_all(ctx.children(), writer)
    }
    fn field(&mut self, field: &Field, ctx: Context<'_>, writer: &mut dyn Write) -> CResult {
        let path = analysis::path(&ctx);
        let offset = self.analysis.at(&ctx)?;
        match field.ty() {
            FieldType::Scalar { definition, .. } | FieldType::Enum(definition) => {
                Self::field_intro(field, writer)?;
                self.emit_all(ctx.children(), writer)?;
                self.scalar(&path, offset, *definition, writer)
            }
            FieldType::Bits(_) => {
                Self::field_intro(field, writer)?;
                self.emit_all(ctx.children(), writer)
            }
            FieldType::Reference(_) => {
                let visit = ctx
                    .referent()
                    .ok_or(CHeaderExportError::InvalidVisit("record referent"))?;
                if matches!(visit.node(), Node::Value(crate::ast::value::Node::Bits(_))) {
                    Self::field_intro(field, writer)?;
                } else {
                    let documentation = match visit.node() {
                        Node::Record(record) => record.spec().documentation(),
                        Node::Union(union) => union.storage().documentation(),
                        Node::Value(crate::ast::value::Node::Opaque(spec)) => spec.documentation(),
                        _ => crate::Documentation::EMPTY,
                    };
                    self.placement(field, &ctx, documentation, writer)?;
                }
                self.emit(visit, writer)
            }
            FieldType::Union(_) => self.emit_all(ctx.children(), writer),
        }
    }
    fn union(
        &mut self,
        value: &UnionDefinition,
        ctx: Context<'_>,
        writer: &mut dyn Write,
    ) -> CResult {
        if ctx.ancestors().is_empty() {
            return self.header(value.storage(), writer);
        }
        let analysis = self.analysis;
        let storage = analysis.union_storage(&ctx)?;
        if storage.is_empty()
            && let Some(field) = ctx.field()
            && matches!(field.ty(), FieldType::Union(_))
        {
            self.placement(field, &ctx, value.storage().documentation(), writer)?;
        }
        for member in storage {
            let (path, field) = (&member.path, &member.field);
            writer.write_all(b"\n")?;
            Fragment::documentation("", field.documentation(), writer)?;
            self.scalar(path, member.offset, field.definition(), writer)?;
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
        writer: &mut dyn Write,
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
        Fragment::documentation(
            &format!("{root}.{path}"),
            field.documentation().with_fallback(documentation),
            writer,
        )?;
        self.output.constants::<64>(
            &format!("{}_RECORD_{}", self.prefix, naming::c_identifier(&path)),
            &[("OFFSET", self.analysis.at(ctx)?), ("SIZE", field.width())],
            writer,
        )
    }

    fn header(&mut self, spec: crate::RecordSpec, writer: &mut dyn Write) -> CResult {
        Fragment::documentation("", spec.documentation(), writer)?;
        self.output.constants::<64>(
            &self.prefix,
            &[("SIZE", spec.size()), ("ALIGNMENT", spec.alignment())],
            writer,
        )?;
        self.output.constants::<32>(
            &self.prefix,
            &[("ENDIAN_LITTLE", 0), ("ENDIAN_BIG", 1)],
            writer,
        )
    }
    fn field_intro(field: &Field, writer: &mut dyn Write) -> CResult {
        writer.write_all(b"\n")?;
        Fragment::documentation("", field.documentation(), writer)
    }
    pub(super) fn scalar(
        &mut self,
        path: &str,
        offset: usize,
        definition: crate::FieldDefinition,
        writer: &mut dyn Write,
    ) -> CResult {
        let name = naming::c_identifier(path);
        let metadata = format!("{}_{}", self.prefix, name);
        self.output
            .constants::<64>(&metadata, &[("OFFSET", offset)], writer)?;
        accessors::scalar_accessors(
            &mut self.output,
            accessors::FieldNames {
                prefix: &self.prefix,
                field: &name,
                metadata: &metadata,
            },
            definition,
            writer,
        )
    }

    pub(super) fn enum_constants(
        &mut self,
        prefix: &str,
        enumeration: EnumDefinition,
        writer: &mut dyn Write,
    ) -> Result<(), CHeaderExportError> {
        for variant in enumeration.variants() {
            Fragment::documentation(variant.name(), variant.documentation(), writer)?;
            self.output.unsigned::<64>(
                &format!("{prefix}_{}", naming::c_identifier(variant.name())),
                variant.value(),
                writer,
            )?;
        }
        Ok(())
    }
}
