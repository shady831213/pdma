use super::super::{CHeaderExportError, CRecordBackend, fragment::Fragment, naming};
use crate::EnumVariantSpec;
use crate::export::{
    record,
    value::{Backend, DocumentationKind, DocumentationOwner, Emit as _},
    view::{
        BitField, Bits, EnumEncoding, Enumeration, Field, Scalar, Storage, StorageField,
        StorageMember,
    },
};
use std::io::Write;
type CResult<T = ()> = Result<T, CHeaderExportError>;

use super::{bit_metadata, bits_metadata, value_prefix};
impl Backend for CRecordBackend<'_> {
    type Domain = record::Domain;
    type Error = CHeaderExportError;
    type Context<'a> = crate::export::record::Context<'a>;
    fn check_entry(
        &mut self,
        ctx: &Self::Context<'_>,
        symbols: &mut crate::export::EntrySymbols,
    ) -> CResult {
        self.names
            .check_entry(ctx.entry_id(), ctx.entry_name(), symbols)
    }
    fn scalar(&mut self, value: Scalar, ctx: Self::Context<'_>, writer: &mut dyn Write) -> CResult {
        Self::value_intro(&ctx, writer)?;
        self.emit_all(ctx.children(), writer)?;
        self.scalar_accessors(
            &ctx.storage_path().unwrap_or_else(|| ctx.path()),
            self.value_offset(&ctx)?,
            value,
            writer,
        )
    }
    fn enumeration(
        &mut self,
        value: Enumeration,
        ctx: Self::Context<'_>,
        writer: &mut dyn Write,
    ) -> CResult {
        if ctx.is_entry() {
            return self.emit_all(ctx.children(), writer);
        }
        match value.encoding() {
            EnumEncoding::Scalar(encoding) => {
                Self::value_intro(&ctx, writer)?;
                self.emit_all(ctx.children(), writer)?;
                self.scalar_accessors(
                    &ctx.storage_path().unwrap_or_else(|| ctx.path()),
                    self.value_offset(&ctx)?,
                    encoding,
                    writer,
                )
            }
            EnumEncoding::Bits { .. } => self.emit_all(ctx.children(), writer),
        }
    }
    fn short_description(
        &mut self,
        text: &str,
        owner: DocumentationOwner<'_>,
        ctx: Self::Context<'_>,
        writer: &mut dyn Write,
    ) -> CResult {
        if ctx.field().is_some()
            || ctx.storage_member().is_some()
            || matches!(
                owner.kind,
                DocumentationKind::Field
                    | DocumentationKind::Record
                    | DocumentationKind::Union
                    | DocumentationKind::Opaque
            )
        {
            return Ok(());
        }
        Fragment::comment(owner.name, text, writer)
    }

    fn description(
        &mut self,
        text: &str,
        owner: DocumentationOwner<'_>,
        ctx: Self::Context<'_>,
        writer: &mut dyn Write,
    ) -> CResult {
        self.short_description(text, owner, ctx, writer)
    }

    fn bits(&mut self, bits: Bits<'_>, ctx: Self::Context<'_>, writer: &mut dyn Write) -> CResult {
        if ctx.field().is_some() {
            Self::value_intro(&ctx, writer)?;
            self.emit_all(ctx.children(), writer)?;
            self.scalar_accessors(
                &ctx.path(),
                self.value_offset(&ctx)?,
                bits.encoding(),
                writer,
            )
        } else {
            bits_metadata(&mut self.output, &self.prefix, bits, writer)?;
            self.emit_all(ctx.children(), writer)
        }
    }
    fn bit_field(
        &mut self,
        field: BitField,
        ctx: Self::Context<'_>,
        writer: &mut dyn Write,
    ) -> CResult {
        if ctx.field().is_none() && ctx.storage_member().is_none() {
            bit_metadata(&mut self.output, &self.prefix, field, writer)?;
        } else {
            Fragment::documentation(field.name(), field.documentation(), writer)?;
        }
        self.emit_all(ctx.children(), writer)
    }
    fn variant(
        &mut self,
        variant: &EnumVariantSpec,
        ctx: Self::Context<'_>,
        writer: &mut dyn Write,
    ) -> CResult {
        if ctx.field().is_some() || ctx.storage_member().is_some() {
            Fragment::documentation(variant.name(), variant.documentation(), writer)?;
        }
        let prefix = if ctx.field().is_some() || ctx.storage_member().is_some() {
            format!(
                "{}_{}",
                self.prefix,
                naming::c_identifier(&ctx.storage_path().unwrap_or_else(|| ctx.path()))
            )
        } else {
            self.prefix.clone()
        };
        let prefix = value_prefix(&prefix, ctx.bit());
        self.output.unsigned::<64>(
            &format!("{prefix}_{}", naming::c_identifier(variant.name())),
            variant.value(),
            writer,
        )
    }
    fn opaque(&mut self, spec: Storage, ctx: Self::Context<'_>, writer: &mut dyn Write) -> CResult {
        self.analysis.check_names(ctx.id())?;
        if ctx.is_entry() {
            self.header(spec, writer)?;
        } else if let Some(field) = ctx.field() {
            self.placement(field, &ctx, spec.documentation(), writer)?;
        }
        self.emit_all(ctx.children(), writer)
    }
    fn storage_member(
        &mut self,
        member: StorageMember<'_>,
        ctx: Self::Context<'_>,
        writer: &mut dyn Write,
    ) -> CResult {
        if member.is_record() {
            let path = ctx
                .storage_path()
                .ok_or(CHeaderExportError::InvalidVisit("storage path"))?;
            Fragment::documentation(member.path(), member.documentation(), writer)?;
            self.output.constants::<64>(
                &format!("{}_RECORD_{}", self.prefix, naming::c_identifier(&path)),
                &[
                    ("OFFSET", self.value_offset(&ctx)?),
                    ("SIZE", member.width()),
                ],
                writer,
            )?;
        }
        self.emit_all(ctx.children(), writer)
    }
}

impl CRecordBackend<'_> {
    fn value_intro(ctx: &record::Context<'_>, writer: &mut dyn Write) -> CResult {
        writer.write_all(b"\n")?;
        let documentation = ctx
            .storage_member()
            .map(StorageMember::documentation)
            .or_else(|| ctx.storage_field().map(StorageField::documentation))
            .or_else(|| ctx.field().map(Field::documentation))
            .unwrap_or(crate::Documentation::EMPTY);
        Fragment::documentation("", documentation, writer)
    }
    fn value_offset(&self, ctx: &record::Context<'_>) -> CResult<usize> {
        let base = if ctx.field().is_some() {
            self.analysis.at(ctx.id())?
        } else {
            0
        };
        if let Some(member) = ctx.storage_member() {
            return base
                .checked_add(member.absolute_offset())
                .ok_or(CHeaderExportError::InvalidVisit("C opaque offset overflow"));
        }
        if let Some(member) = ctx.storage_field() {
            return base
                .checked_sub(ctx.field().map_or(0, Field::offset))
                .and_then(|base| base.checked_add(member.offset()))
                .ok_or(CHeaderExportError::InvalidVisit("C union offset overflow"));
        }
        Ok(base)
    }
}
