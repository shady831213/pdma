use super::super::{CHeaderExportError, CRegisterBackend, fragment::Fragment, naming};
use crate::EnumVariantSpec;
use crate::export::{
    OutputMetadata, register,
    value::{Backend, DocumentationOwner, Emit as _},
    view::{BitField, Bits, Storage},
};
use std::io::Write;
type CResult<T = ()> = Result<T, CHeaderExportError>;

use super::{bit_metadata, bits_metadata, value_prefix};
impl Backend for CRegisterBackend {
    type Domain = register::Domain;
    type Error = CHeaderExportError;
    type Context<'a> = crate::export::register::Context<'a>;
    fn expression(
        &mut self,
        value: crate::export::view::Expression<'_>,
        ctx: Self::Context<'_>,
        writer: &mut dyn Write,
    ) -> CResult {
        if ctx.parent_kind() != Some(crate::export::Kind::LayoutProperty) {
            return Ok(());
        }
        if ctx.bank().is_some() {
            write!(writer, "{}", value.evaluate(&[], &[])?)?;
        } else {
            write!(
                writer,
                "{}",
                value.format(&mut super::super::register::Syntax)?
            )?;
        }
        Ok(())
    }
    fn short_description(
        &mut self,
        text: &str,
        owner: DocumentationOwner<'_>,
        ctx: Self::Context<'_>,
        writer: &mut dyn Write,
    ) -> CResult {
        if ctx.repeated_documentation() {
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
        if ctx.register().is_none() {
            bits_metadata(&mut self.output, &self.prefix, bits, writer)?;
        }
        self.emit_all(ctx.children(), writer)
    }
    fn bit_field(
        &mut self,
        field: BitField<'_>,
        ctx: Self::Context<'_>,
        writer: &mut dyn Write,
    ) -> CResult {
        if ctx.register().is_none() {
            bit_metadata(&mut self.output, &self.prefix, field, writer)?;
        }
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
    fn opaque(&mut self, spec: Storage, _: Self::Context<'_>, writer: &mut dyn Write) -> CResult {
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
}
impl crate::export::OutputBackend for CRegisterBackend {
    fn metadata(&mut self, ctx: &Self::Context<'_>) -> CResult<OutputMetadata> {
        let logical_name = self.names.reserve_unit(ctx.root_id(), ctx.root_name())?;
        Ok(OutputMetadata {
            logical_name,
            media_type: "text/x-c; charset=utf-8".into(),
        })
    }
}
