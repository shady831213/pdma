use super::super::super::layout::register_name;
use super::super::SvRegisterBackend;
use crate::{
    EnumVariantSpec,
    export::{
        register,
        value::{self, Backend, Emit as _},
        view::{Bits, Storage},
    },
};
use std::io::Write;
impl Backend for SvRegisterBackend {
    fn check_entry(
        &mut self,
        ctx: &Self::Context<'_>,
        symbols: &mut crate::export::EntrySymbols,
    ) -> Result<(), Self::Error> {
        self.output
            .check_entry(ctx.entry_id(), ctx.entry_name(), symbols)
    }
    type Error = std::io::Error;
    type Domain = register::Domain;
    type Context<'a> = crate::export::register::Context<'a>;
    fn expression(
        &mut self,
        value: crate::export::view::Expression<'_>,
        ctx: Self::Context<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        if ctx.parent_kind() == Some(crate::export::Kind::LayoutProperty) {
            write!(
                writer,
                "{}",
                value.format(self).map_err(std::io::Error::other)?
            )?;
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
            self.output.package.constant(
                "BIT_WIDTH",
                "int unsigned",
                format_args!("32'd{}", bits.bit_width()),
                writer,
            )?;
            self.output.package.constant(
                "DECLARED_MASK",
                "logic [63:0]",
                format_args!("64'h{:016x}", bits.declared_mask()),
                writer,
            )?;
        }
        self.emit_all(ctx.children(), writer)
    }
    fn bit_field(
        &mut self,
        field: crate::export::view::BitField,
        ctx: Self::Context<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        if ctx.register().is_some() {
            let prefix = super::super::super::layout::register_name(ctx.register_segments());
            let name = format!("{prefix}_{}", field.name().to_ascii_uppercase());
            self.output.package.constant(
                &format!("{name}_LSB"),
                "int unsigned",
                format_args!("32'd{}", field.lsb()),
                writer,
            )?;
            self.output.package.constant(
                &format!("{name}_BITS"),
                "int unsigned",
                format_args!("32'd{}", field.bit_width()),
                writer,
            )?;
        }
        self.emit_all(ctx.children(), writer)
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
                "logic [63:0]",
                format_args!("64'd{}", variant.value()),
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
        super::super::super::sv::comment(text, writer)
    }
}
