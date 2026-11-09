use hw_model_schema::{
    Endian, EnumVariantSpec,
    export::{
        Kind, OutputMetadata, SymbolScope, Visit,
        record::{self, Context},
        value::{self, Emit},
        view::{BitField, Bits, EnumEncoding, Enumeration, Field, Reference, Scalar, Union},
    },
    traversal::NodeId,
};
use std::io::{Error, Write};

#[derive(Default)]
pub(super) struct SystemVerilog {
    names: SymbolScope<NodeId>,
}

impl value::Backend for SystemVerilog {
    type Error = Error;
    type Domain = record::Domain;
    type Context<'a> = Context<'a>;

    fn scalar(
        &mut self,
        value: Scalar,
        ctx: Context<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Error> {
        Self::accessor(&ctx, value.width(), value.endian(), writer)?;
        self.emit_all(ctx.children(), writer)
    }

    fn enumeration(
        &mut self,
        value: Enumeration,
        ctx: Context<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Error> {
        if let EnumEncoding::Scalar(encoding) = value.encoding() {
            Self::accessor(&ctx, encoding.width(), encoding.endian(), writer)?;
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
        Self::accessor(&ctx, encoding.width(), encoding.endian(), writer)?;
        self.emit_all(ctx.children(), writer)
    }
    fn variant(
        &mut self,
        value: &EnumVariantSpec,
        ctx: Context<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        let field = ctx.path().replace('.', "_");
        let bit = ctx.bit().map_or(String::new(), |name| format!("_{name}"));
        let name =
            format!("{}_{field}{bit}_{}", ctx.root_name(), value.name()).to_ascii_uppercase();
        writeln!(
            writer,
            "  localparam logic [63:0] {name} = 64'd{};",
            value.value()
        )?;
        Ok(())
    }

    fn bit_field<'a>(
        &mut self,
        value: BitField<'a>,
        ctx: Context<'a>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        let field = ctx.path().replace('.', "_");
        let name = format!("{}_{field}_{}", ctx.root_name(), value.name()).to_ascii_uppercase();
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

impl record::Backend for SystemVerilog {
    fn root(&mut self, ctx: Context<'_>, writer: &mut dyn Write) -> Result<(), Self::Error> {
        let name = ctx.root_name();
        let upper = name.to_ascii_uppercase();
        if let Some((size, alignment)) = ctx.root_extent() {
            writeln!(writer, "\n  localparam int unsigned {upper}_SIZE = {size};")?;
            writeln!(
                writer,
                "  localparam int unsigned {upper}_ALIGNMENT = {alignment};"
            )?;
            writeln!(writer, "  typedef logic [{upper}_SIZE*8-1:0] {name}_t;")?;
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
            Self::accessor(&ctx, field.width(), Endian::Little, writer)?;
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
        if target.kind() == Kind::Bits {
            self.emit(target, writer)
        } else if let Some(field) = ctx.field() {
            Self::accessor(&ctx, field.width(), Endian::Little, writer)
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
        let record = ctx.root_name();
        let name = field.name();
        let constant = format!("{record}_{name}").to_ascii_uppercase();
        writeln!(
            writer,
            "  localparam int unsigned {constant}_OFFSET = {};",
            field.offset()
        )?;
        writeln!(
            writer,
            "  localparam int unsigned {constant}_SIZE = {};",
            field.width()
        )?;
        self.emit_all(ctx.children(), writer)
    }
}

impl SystemVerilog {
    fn accessor(
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
            super::codec::accessors(ctx.root_name(), field.name(), bytes, endian, writer)?;
        }
        Ok(())
    }
}

impl hw_model_schema::export::OutputBackend for SystemVerilog {
    fn metadata(&mut self, ctx: &Context<'_>) -> Result<OutputMetadata, Self::Error> {
        self.names
            .insert(ctx.root_name(), ctx.root_id().clone())
            .map_err(Error::other)?;
        Ok(OutputMetadata {
            logical_name: ctx.root_name().to_owned(),
            media_type: "text/x-systemverilog".into(),
        })
    }
}
