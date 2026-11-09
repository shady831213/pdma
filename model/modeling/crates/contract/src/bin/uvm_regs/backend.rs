use hw_model_schema::{
    EnumVariantSpec,
    export::{
        OutputBackend, OutputMetadata, SymbolScope,
        expression::ExpressionSyntax,
        register::{self, Context},
        value::{self, DocumentationOwner, Emit},
        view,
    },
    traversal::NodeId,
};
use std::io::{Error, Write};

#[derive(Default)]
pub(super) struct Uvm {
    names: SymbolScope<NodeId>,
    pub(super) variants: SymbolScope<NodeId>,
    pub(super) initialization: Vec<u8>,
}

impl value::Backend for Uvm {
    type Error = Error;
    type Domain = register::Domain;
    type Context<'a> = Context<'a>;

    fn bits(
        &mut self,
        _: view::Bits<'_>,
        ctx: Context<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Error> {
        self.emit_all(ctx.children(), writer)
    }

    fn bit_field<'a>(
        &mut self,
        field: view::BitField<'a>,
        ctx: Context<'a>,
        writer: &mut dyn Write,
    ) -> Result<(), Error> {
        self.field((field.name(), field.lsb(), field.bit_width()), &ctx, writer)
    }

    fn variant(
        &mut self,
        value: &EnumVariantSpec,
        ctx: Context<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Error> {
        let field = ctx
            .bit()
            .ok_or_else(|| Error::other("enum without field owner"))?;
        let name = format!("{field}_{}", value.name()).to_ascii_uppercase();
        self.variants
            .insert(&name, ctx.id().clone())
            .map_err(Error::other)?;
        writeln!(
            writer,
            "    localparam logic [63:0] {name} = 64'd{};",
            value.value()
        )
    }

    fn short_description(
        &mut self,
        text: &str,
        _: DocumentationOwner<'_>,
        _: Context<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Error> {
        for line in text.lines() {
            writeln!(writer, "    // {line}")?;
        }
        Ok(())
    }

    fn description(
        &mut self,
        text: &str,
        owner: DocumentationOwner<'_>,
        ctx: Context<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Error> {
        self.short_description(text, owner, ctx, writer)
    }

    fn expression(
        &mut self,
        value: view::Expression<'_>,
        _: Context<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Error> {
        writer.write_all(value.format(&mut Syntax).map_err(Error::other)?.as_bytes())
    }
}

impl OutputBackend for Uvm {
    fn metadata(&mut self, ctx: &Context<'_>) -> Result<OutputMetadata, Error> {
        self.names
            .insert(ctx.root_name(), ctx.root_id().clone())
            .map_err(Error::other)?;
        Ok(OutputMetadata {
            logical_name: ctx.root_name().to_owned(),
            media_type: "text/x-systemverilog".into(),
        })
    }
}

struct Syntax;
impl ExpressionSyntax for Syntax {
    fn parameter(&mut self, name: &str) -> String {
        format!("p_{name}")
    }
    fn literal(&mut self, value: usize) -> String {
        format!("64'd{value}")
    }
}
