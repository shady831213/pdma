use super::{CHeaderExportError, CRecordBackend, CRegisterBackend, fragment::Fragment, naming};
use crate::ast::BitsDefinition;
use crate::export::{
    OutputMetadata, record, register,
    value::{Backend, DocumentationKind, DocumentationOwner},
};
use crate::{BitsFieldSpec, EnumVariantSpec, RecordSpec};
use std::io::Write;
type CResult<T = ()> = Result<T, CHeaderExportError>;

impl Backend for CRecordBackend<'_> {
    type Domain = record::Domain;
    type Error = CHeaderExportError;
    type Context<'a> = crate::traversal::record::Context<'a>;
    fn short_description(
        &mut self,
        text: &str,
        owner: DocumentationOwner<'_>,
        ctx: Self::Context<'_>,
        writer: &mut dyn Write,
    ) -> CResult {
        if ctx.field().is_some()
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

    fn bits(
        &mut self,
        bits: &BitsDefinition,
        ctx: Self::Context<'_>,
        writer: &mut dyn Write,
    ) -> CResult {
        if let Some(field) = ctx.field() {
            self.emit_all(ctx.children(), writer)?;
            let definition = crate::FieldDefinition::new(bits.scalar())
                .with_endian(field.endian())
                .with_width(field.width());
            self.scalar(
                &super::analysis::path(&ctx),
                self.analysis.at(&ctx)?,
                definition,
                writer,
            )
        } else {
            bits_metadata(&mut self.output, &self.prefix, bits, writer)?;
            self.emit_all(ctx.children(), writer)
        }
    }
    fn bit_field(
        &mut self,
        field: &BitsFieldSpec,
        ctx: Self::Context<'_>,
        writer: &mut dyn Write,
    ) -> CResult {
        if ctx.field().is_none() {
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
        if ctx.field().is_some() {
            Fragment::documentation(variant.name(), variant.documentation(), writer)?;
        }
        let prefix = if ctx.field().is_some() {
            format!(
                "{}_{}",
                self.prefix,
                naming::c_identifier(&super::analysis::path(&ctx))
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
    fn opaque(
        &mut self,
        _: &RecordSpec,
        ctx: Self::Context<'_>,
        writer: &mut dyn Write,
    ) -> CResult {
        self.opaque_storage(&ctx, writer)
    }
    fn metadata(&mut self, ctx: &Self::Context<'_>) -> CResult<OutputMetadata> {
        let logical_name = self.names.reserve_unit(ctx.unit())?;
        Ok(OutputMetadata {
            logical_name,
            media_type: "text/x-c; charset=utf-8".into(),
        })
    }
}
impl Backend for CRegisterBackend {
    type Domain = register::Domain;
    type Error = CHeaderExportError;
    type Context<'a> = crate::traversal::register::Context<'a>;
    fn short_description(
        &mut self,
        text: &str,
        owner: DocumentationOwner<'_>,
        ctx: Self::Context<'_>,
        writer: &mut dyn Write,
    ) -> CResult {
        if ctx.repeated_documentation()
            || self.geometry.section != super::register::GeometrySection::Documentation
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
    fn bits(
        &mut self,
        bits: &BitsDefinition,
        ctx: Self::Context<'_>,
        writer: &mut dyn Write,
    ) -> CResult {
        bits_metadata(&mut self.output, &self.prefix, bits, writer)?;
        self.emit_all(ctx.children(), writer)
    }
    fn bit_field(
        &mut self,
        field: &BitsFieldSpec,
        ctx: Self::Context<'_>,
        writer: &mut dyn Write,
    ) -> CResult {
        bit_metadata(&mut self.output, &self.prefix, field, writer)?;
        self.emit_all(ctx.children(), writer)
    }
    fn variant(
        &mut self,
        variant: &EnumVariantSpec,
        ctx: Self::Context<'_>,
        writer: &mut dyn Write,
    ) -> CResult {
        let prefix = if ctx.register().is_some() {
            self.register_name(&ctx)?
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
    fn opaque(
        &mut self,
        spec: &RecordSpec,
        _: Self::Context<'_>,
        writer: &mut dyn Write,
    ) -> CResult {
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
    fn metadata(&mut self, ctx: &Self::Context<'_>) -> CResult<OutputMetadata> {
        let logical_name = self.names.reserve_unit(ctx.unit())?;
        Ok(OutputMetadata {
            logical_name,
            media_type: "text/x-c; charset=utf-8".into(),
        })
    }
}
fn bits_metadata(
    output: &mut Fragment,
    prefix: &str,
    bits: &BitsDefinition,
    writer: &mut dyn Write,
) -> CResult {
    output.constants::<64>(prefix, &[("SIZE", bits.scalar().width())], writer)?;
    output.constants::<32>(prefix, &[("BIT_WIDTH", bits.bit_width())], writer)?;
    output.constants::<64>(prefix, &[("DECLARED_MASK", bits.declared_mask())], writer)
}
fn bit_metadata(
    output: &mut Fragment,
    prefix: &str,
    field: &BitsFieldSpec,
    writer: &mut dyn Write,
) -> CResult {
    let prefix = format!("{prefix}_{}", naming::c_identifier(field.name()));
    output.constants::<32>(
        &prefix,
        &[("LSB", field.lsb()), ("BITS", field.bit_width())],
        writer,
    )
}
fn value_prefix(prefix: &str, bit: Option<&str>) -> String {
    bit.map_or_else(
        || prefix.into(),
        |bit| format!("{prefix}_{}", naming::c_identifier(bit)),
    )
}
