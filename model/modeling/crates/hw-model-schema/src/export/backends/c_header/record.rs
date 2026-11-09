use crate::export::backends::c_header::fragment::Fragment;
use crate::export::record::Context;
use std::io::Write;
mod accessors;

use super::{CHeaderExportError, CRecordBackend, naming};
use crate::export::record::Backend;
use crate::export::value::Emit as _;
use crate::export::view::{Field, Record, Reference, Scalar, Storage, Union};
type CResult<T = ()> = Result<T, CHeaderExportError>;
impl Backend for CRecordBackend<'_> {
    fn root(&mut self, ctx: Context<'_>, writer: &mut dyn Write) -> CResult {
        self.prefix = self.names.unit_name(ctx.root_id(), ctx.root_name()).1;
        self.output = Fragment::default();
        self.emit(ctx.current(), writer)
    }
    fn record(&mut self, value: Record<'_>, ctx: Context<'_>, writer: &mut dyn Write) -> CResult {
        if ctx.is_root() {
            self.header(value.spec(), writer)?;
        } else if let Some(field) = ctx.field() {
            self.placement(field, &ctx, value.spec().documentation(), writer)?;
        }
        self.emit_all(ctx.children(), writer)
    }
    fn reference<'a>(
        &mut self,
        _: Reference<'a>,
        target: crate::export::Visit<Context<'a>>,
        ctx: Context<'a>,
        writer: &mut dyn Write,
    ) -> CResult {
        if let Some(field) = ctx.field() {
            self.placement(field, &ctx, target.documentation(), writer)?;
        }
        Ok(())
    }
    fn union(&mut self, value: Union<'_>, ctx: Context<'_>, writer: &mut dyn Write) -> CResult {
        if ctx.is_root() {
            return self.header(value.storage(), writer);
        }
        let storage = ctx.union_storage();
        if storage.len() == 0
            && ctx.parent_kind() == Some(crate::export::Kind::Field)
            && let Some(field) = ctx.field()
        {
            self.placement(field, &ctx, value.storage().documentation(), writer)?;
        }
        self.emit_all(storage, writer)
    }
}

impl CRecordBackend<'_> {
    pub(super) fn placement(
        &mut self,
        field: Field<'_>,
        ctx: &Context<'_>,
        documentation: crate::Documentation,
        writer: &mut dyn Write,
    ) -> CResult {
        let path = ctx.path();
        let root = ctx
            .record_name()
            .ok_or(CHeaderExportError::InvalidVisit("record reference parent"))?;
        Fragment::documentation(
            &format!("{root}.{path}"),
            field.documentation().with_fallback(documentation),
            writer,
        )?;
        self.output.constants::<64>(
            &format!("{}_RECORD_{}", self.prefix, naming::c_identifier(&path)),
            &[
                ("OFFSET", self.analysis.at(ctx.id())?),
                ("SIZE", field.width()),
            ],
            writer,
        )
    }

    pub(super) fn header(&mut self, spec: Storage, writer: &mut dyn Write) -> CResult {
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
    pub(super) fn scalar_accessors(
        &mut self,
        path: &str,
        offset: usize,
        definition: Scalar,
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
}
