use super::{VerylExportError, VerylRecordBackend, VerylRegisterBackend, package::register_name};
use crate::{
    EnumVariantSpec,
    export::{
        OutputMetadata, record, register,
        value::{self, Backend, Emit as _},
        view::{Bits, Storage},
    },
};
use std::io::Write;
impl Backend for VerylRecordBackend {
    type Error = VerylExportError;
    type Domain = record::Domain;
    type Context<'a> = crate::export::record::Context<'a>;
    fn bits<'a>(
        &mut self,
        bits: Bits<'a>,
        ctx: Self::Context<'a>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        if ctx.is_root() {
            self.output
                .package
                .extent(bits.scalar().width(), bits.scalar().width(), writer)?;
            self.output
                .package
                .constant("BIT_WIDTH", "u32", bits.bit_width(), writer)?;
            self.output.package.constant(
                "DECLARED_MASK",
                "u64",
                format_args!("64'h{:016x}", bits.declared_mask()),
                writer,
            )?;
        }
        self.emit_all(ctx.children(), writer)
    }
    fn opaque(
        &mut self,
        _: Storage,
        ctx: Self::Context<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        self.extent(&ctx, writer)
    }
    fn short_description(
        &mut self,
        text: &str,
        _: value::DocumentationOwner<'_>,
        _: Self::Context<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        super::package::Package::comment(text, writer)
    }
    fn description(
        &mut self,
        text: &str,
        _: value::DocumentationOwner<'_>,
        _: Self::Context<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        super::package::Package::comment(text, writer)
    }
}
impl crate::export::OutputBackend for VerylRecordBackend {
    fn metadata(&mut self, ctx: &Self::Context<'_>) -> Result<OutputMetadata, Self::Error> {
        self.output.metadata(ctx.root_id(), ctx.root_name())
    }
}
impl Backend for VerylRegisterBackend {
    type Error = VerylExportError;
    type Domain = register::Domain;
    type Context<'a> = crate::export::register::Context<'a>;
    fn expression(
        &mut self,
        value: crate::export::view::Expression<'_>,
        ctx: Self::Context<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        if ctx.parent_kind() == Some(crate::export::Kind::LayoutProperty) {
            write!(writer, "{}", value.format(&mut super::register::Syntax)?)?;
        }
        Ok(())
    }
    fn bits<'a>(
        &mut self,
        bits: Bits<'a>,
        ctx: Self::Context<'a>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        if ctx.register().is_none() {
            self.output
                .package
                .extent(bits.scalar().width(), bits.scalar().width(), writer)?;
            self.output
                .package
                .constant("BIT_WIDTH", "u32", bits.bit_width(), writer)?;
            self.output.package.constant(
                "DECLARED_MASK",
                "u64",
                format_args!("64'h{:016x}", bits.declared_mask()),
                writer,
            )?;
        }
        self.emit_all(ctx.children(), writer)
    }
    fn bit_field(
        &mut self,
        field: crate::export::view::BitField<'_>,
        ctx: Self::Context<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        if ctx.register().is_some() {
            self.field_metadata(field, &ctx, writer)
        } else {
            self.emit_all(ctx.children(), writer)
        }
    }
    fn opaque(
        &mut self,
        value: Storage,
        _: Self::Context<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        self.output
            .package
            .extent(value.size(), value.alignment(), writer)
    }
    fn variant(
        &mut self,
        variant: &EnumVariantSpec,
        ctx: Self::Context<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        if let (Some(bit), Some(_)) = (ctx.bit(), ctx.register()) {
            let prefix = register_name(ctx.register_segments());
            self.output.package.constant(
                &format!(
                    "{prefix}_{}_{}",
                    bit.to_ascii_uppercase(),
                    variant.name().to_ascii_uppercase()
                ),
                "u64",
                variant.value(),
                writer,
            )?;
        }
        Ok(())
    }
    fn short_description(
        &mut self,
        text: &str,
        _: value::DocumentationOwner<'_>,
        _: Self::Context<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        super::package::Package::comment(text, writer)
    }
    fn description(
        &mut self,
        text: &str,
        _: value::DocumentationOwner<'_>,
        _: Self::Context<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        super::package::Package::comment(text, writer)
    }
}
impl crate::export::OutputBackend for VerylRegisterBackend {
    fn metadata(&mut self, ctx: &Self::Context<'_>) -> Result<OutputMetadata, Self::Error> {
        self.output.metadata(ctx.root_id(), ctx.root_name())
    }
}
