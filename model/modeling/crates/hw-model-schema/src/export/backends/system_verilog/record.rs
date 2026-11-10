use super::SvRecordBackend;
use crate::{
    Endian,
    export::{
        Kind, Visit,
        record::{self, Context},
        value::Emit,
        view::{Field, Reference, Union},
    },
};
use std::io::{Error, Write};
mod codec;
mod values;
impl record::Backend for SvRecordBackend {
    fn entry(&mut self, ctx: Context<'_>, writer: &mut dyn Write) -> Result<(), Error> {
        self.symbols.begin(ctx.entry_id());
        if let Some((size, alignment)) = ctx.entry_extent() {
            self.image(&ctx, size, alignment, writer)?;
        }
        self.emit(ctx.current(), writer)
    }

    fn union<'a>(
        &mut self,
        _: Union<'a>,
        ctx: Context<'a>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        if let Some(field) = ctx.field() {
            self.accessor(&ctx, field.width(), Endian::Little, writer)?;
        }
        self.emit_all(ctx.children(), writer)
    }

    fn reference<'a>(
        &mut self,
        _: Reference<'a>,
        target: Visit<Context<'a>>,
        ctx: Context<'a>,
        writer: &mut dyn Write,
    ) -> Result<(), Error> {
        if matches!(target.kind(), Kind::Bits | Kind::Enum)
            || target.definition_origin() == crate::ast::Origin::Inline
        {
            self.emit(target, writer)
        } else if let Some(field) = ctx.field() {
            self.accessor(&ctx, field.width(), Endian::Little, writer)
        } else {
            Ok(())
        }
    }

    fn field<'a>(
        &mut self,
        field: Field<'a>,
        ctx: Context<'a>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        if ctx.ancestor_kinds().any(|kind| kind == Kind::Case) {
            return self.emit_all(ctx.children(), writer);
        }
        let record = self.names.get(ctx.entry_id(), ctx.entry_name());
        let name = field.name();
        let constant = format!("{record}_{name}").to_ascii_uppercase();
        self.symbols
            .declare(&format!("{constant}_OFFSET"), ctx.entry_id())?;
        self.symbols
            .declare(&format!("{constant}_SIZE"), ctx.entry_id())?;
        writeln!(
            writer,
            "  localparam logic [63:0] {constant}_OFFSET = {};",
            field.offset()
        )?;
        writeln!(
            writer,
            "  localparam logic [63:0] {constant}_SIZE = {};",
            field.width()
        )?;
        self.emit_all(ctx.children(), writer)
    }
}

impl SvRecordBackend {
    fn accessor(
        &mut self,
        ctx: &Context<'_>,
        bytes: usize,
        endian: Endian,
        writer: &mut dyn Write,
    ) -> Result<(), Error> {
        if ctx
            .ancestor_kinds()
            .any(|kind| matches!(kind, Kind::Case | Kind::BitField))
        {
            return Ok(());
        }
        if let Some(field) = ctx.field() {
            Self::check_size(bytes)?;
            let name = self.names.get(ctx.entry_id(), ctx.entry_name());
            self.symbols
                .declare(&format!("{name}_get_{}", field.name()), ctx.entry_id())?;
            self.symbols
                .declare(&format!("{name}_set_{}", field.name()), ctx.entry_id())?;
            codec::accessors(&name, field.name(), bytes, endian, writer)?;
        }
        Ok(())
    }
    fn check_size(size: usize) -> Result<(), Error> {
        if size == 0 || size > ((1_usize << 28) - 1) {
            return Err(Error::new(
                std::io::ErrorKind::InvalidInput,
                "SV byte image must have 1..=268435455 bytes",
            ));
        }
        Ok(())
    }
    fn image(
        &mut self,
        ctx: &Context<'_>,
        size: usize,
        align: usize,
        w: &mut dyn Write,
    ) -> Result<(), Error> {
        Self::check_size(size)?;
        let name = self.names.get(ctx.entry_id(), ctx.entry_name());
        let upper = name.to_ascii_uppercase();
        for symbol in [
            format!("{upper}_SIZE"),
            format!("{upper}_ALIGNMENT"),
            format!("{name}_t"),
        ] {
            self.symbols.declare(&symbol, ctx.entry_id())?;
        }
        writeln!(
            w,
            "\n  localparam logic [63:0] {upper}_SIZE = 64'd{size};\n  localparam logic [63:0] {upper}_ALIGNMENT = 64'd{align};\n  typedef logic [{upper}_SIZE*8-1:0] {name}_t;"
        )
    }
}
