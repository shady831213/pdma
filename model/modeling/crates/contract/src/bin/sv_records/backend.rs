use hw_model_schema::{
    BitsFieldSpec, Endian, EnumVariantSpec,
    analysis::Layout,
    ast::{Field, FieldType, UnionDefinition},
    export::{
        OutputMetadata, SymbolScope, record,
        value::{self, Backend as _},
    },
    traversal::{NodeId, record::Context},
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

    fn metadata(&mut self, ctx: &Context<'_>) -> Result<OutputMetadata, Self::Error> {
        self.names
            .insert(&ctx.unit().name, ctx.unit().id.clone())
            .map_err(Error::other)?;
        Ok(OutputMetadata {
            logical_name: ctx.unit().name.clone(),
            media_type: "text/x-systemverilog".into(),
        })
    }

    fn variant(
        &mut self,
        value: &EnumVariantSpec,
        ctx: Context<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        let field = ctx.field().map_or("", Field::name);
        let bit = ctx.bit().map_or(String::new(), |name| format!("_{name}"));
        let name =
            format!("{}_{field}{bit}_{}", ctx.unit().name, value.name()).to_ascii_uppercase();
        writeln!(
            writer,
            "  localparam logic [63:0] {name} = 64'd{};",
            value.value()
        )?;
        Ok(())
    }

    fn bit_field<'a>(
        &mut self,
        value: &'a BitsFieldSpec,
        ctx: Context<'a>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        let field = ctx.field().map_or("", Field::name);
        let name = format!("{}_{field}_{}", ctx.unit().name, value.name()).to_ascii_uppercase();
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
        let name = &ctx.unit().name;
        let upper = name.to_ascii_uppercase();
        if let Layout::Record(layout) = &ctx.unit().body {
            writeln!(
                writer,
                "\n  localparam int unsigned {upper}_SIZE = {};",
                layout.size()
            )?;
            writeln!(
                writer,
                "  localparam int unsigned {upper}_ALIGNMENT = {};",
                layout.alignment()
            )?;
            writeln!(writer, "  typedef logic [{upper}_SIZE*8-1:0] {name}_t;")?;
        }
        self.emit(ctx.current(), writer)
    }

    fn union<'a>(
        &mut self,
        _: &'a UnionDefinition,
        _: Context<'a>,
        _: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        // Union storage is a byte image; referenced alternatives are independent schema roots.
        Ok(())
    }

    fn field<'a>(
        &mut self,
        field: &'a Field,
        ctx: Context<'a>,
        writer: &mut dyn Write,
    ) -> Result<(), Self::Error> {
        let record = &ctx.unit().name;
        let name = field.name();
        let constant = format!("{record}_{name}").to_ascii_uppercase();
        let width = field.width() * 8;
        let scalar = match field.ty() {
            FieldType::Scalar { .. } | FieldType::Enum(_) | FieldType::Bits(_) => true,
            FieldType::Reference(_) | FieldType::Union(_) => false,
        };
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
        writeln!(
            writer,
            "  function automatic logic [{}:0] {record}_get_{name}(input {record}_t record_value);",
            width - 1
        )?;
        for index in 0..field.width() {
            let target = match (scalar, field.endian()) {
                (true, Endian::Big) => field.width() - index - 1,
                (true | false, Endian::Little) | (false, Endian::Big) => index,
            } * 8;
            writeln!(
                writer,
                "    {record}_get_{name}[{target} +: 8] = record_value[({constant}_OFFSET+{index})*8 +: 8];"
            )?;
        }
        writeln!(writer, "  endfunction")?;
        writeln!(
            writer,
            "  function automatic void {record}_set_{name}(inout {record}_t record_value, input logic [{}:0] value);",
            width - 1
        )?;
        for index in 0..field.width() {
            let source = match (scalar, field.endian()) {
                (true, Endian::Big) => field.width() - index - 1,
                (true | false, Endian::Little) | (false, Endian::Big) => index,
            } * 8;
            writeln!(
                writer,
                "    record_value[({constant}_OFFSET+{index})*8 +: 8] = value[{source} +: 8];"
            )?;
        }
        writeln!(writer, "  endfunction")?;
        self.emit_all(ctx.children(), writer)
    }
}
