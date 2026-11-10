use super::SvRecordBackend;
use crate::export::value::{self, DocumentationOwner};
use crate::{
    EnumVariantSpec,
    export::{
        record::{self, Context},
        value::Emit,
        view::{BitField, Bits, EnumEncoding, Enumeration, Scalar},
    },
};
use std::io::{Error, Write};
impl value::Backend for SvRecordBackend {
    type Error = Error;
    type Domain = record::Domain;
    type Context<'a> = Context<'a>;

    fn check_entry(
        &mut self,
        ctx: &Context<'_>,
        symbols: &mut crate::export::EntrySymbols,
    ) -> Result<(), Error> {
        if let Some((size, _)) = ctx.entry_extent() {
            Self::check_size(size)?;
        }
        let name = self.names.get(ctx.entry_id(), ctx.entry_name());
        super::super::super::sv::valid_name(&format!("{name}_t"))?;
        symbols
            .declare(
                &name.to_ascii_uppercase(),
                ctx.entry_id()
                    .source_path()
                    .unwrap_or_else(|| ctx.entry_name()),
            )
            .map_err(Error::other)
    }
    fn short_description(
        &mut self,
        text: &str,
        _: DocumentationOwner<'_>,
        _: Context<'_>,
        w: &mut dyn Write,
    ) -> Result<(), Error> {
        super::super::super::sv::comment(text, w)
    }
    fn scalar(
        &mut self,
        value: Scalar,
        ctx: Context<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Error> {
        self.accessor(&ctx, value.width(), value.endian(), writer)?;
        self.emit_all(ctx.children(), writer)
    }

    fn enumeration(
        &mut self,
        value: Enumeration,
        ctx: Context<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Error> {
        if let EnumEncoding::Scalar(encoding) = value.encoding() {
            if ctx.is_entry() {
                self.image(&ctx, encoding.width(), encoding.width(), writer)?;
            }
            self.accessor(&ctx, encoding.width(), encoding.endian(), writer)?;
        }
        self.emit_all(ctx.children(), writer)
    }

    fn bits(
        &mut self,
        value: Bits<'_>,
        ctx: Context<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Error> {
        let encoding = value.encoding();
        if ctx.is_entry() {
            self.image(&ctx, encoding.width(), encoding.width(), writer)?;
        }
        self.accessor(&ctx, encoding.width(), encoding.endian(), writer)?;
        self.emit_all(ctx.children(), writer)
    }
    fn variant(
        &mut self,
        value: &EnumVariantSpec,
        ctx: Context<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        let mut parts = vec![
            self.names.get(ctx.entry_id(), ctx.entry_name()),
            ctx.path().replace('.', "_"),
        ];
        if let Some(bit) = ctx.bit() {
            parts.push(bit.into());
        }
        parts.push(value.name().into());
        let name = parts
            .into_iter()
            .filter(|p| !p.is_empty())
            .collect::<Vec<_>>()
            .join("_")
            .to_ascii_uppercase();
        self.symbols.declare(&name, ctx.entry_id())?;
        writeln!(
            writer,
            "  localparam logic [63:0] {name} = 64'd{};",
            value.value()
        )?;
        Ok(())
    }

    fn bit_field(
        &mut self,
        value: BitField,
        ctx: Context<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        let name = [
            self.names.get(ctx.entry_id(), ctx.entry_name()),
            ctx.path().replace('.', "_"),
            value.name().into(),
        ]
        .into_iter()
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("_")
        .to_ascii_uppercase();
        self.symbols
            .declare(&format!("{name}_LSB"), ctx.entry_id())?;
        self.symbols
            .declare(&format!("{name}_WIDTH"), ctx.entry_id())?;
        writeln!(
            writer,
            "  localparam int unsigned {name}_LSB = {};",
            value.lsb()
        )?;
        writeln!(
            writer,
            "  localparam int unsigned {name}_WIDTH = {};",
            value.bit_width()
        )?;
        self.emit_all(ctx.children(), writer)
    }
}
