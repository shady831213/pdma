use super::Uvm;
use hw_model_schema::{
    EnumVariantSpec,
    export::{
        expression::ExpressionSyntax,
        register::{self, Context},
        value::{self, DocumentationOwner, Emit},
        view,
    },
};
use std::io::{Error, Write};

impl value::Backend for Uvm {
    type Error = Error;
    type Domain = register::Domain;
    type Context<'a> = Context<'a>;

    fn check_entry(
        &mut self,
        ctx: &Context<'_>,
        symbols: &mut hw_model_schema::export::EntrySymbols,
    ) -> Result<(), Error> {
        symbols
            .declare(
                ctx.entry_name(),
                ctx.entry_id()
                    .source_path()
                    .unwrap_or_else(|| ctx.entry_name()),
            )
            .map_err(Error::other)
    }

    fn bit_field(
        &mut self,
        field: view::BitField,
        ctx: Context<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Error> {
        writeln!(writer, "    uvm_reg_field {}_field;", field.name())?;
        let reg = ctx
            .register()
            .ok_or_else(|| Error::other("missing register owner"))?;
        super::fields::ral_field(
            reg,
            (field.name(), field.lsb(), field.bit_width()),
            &mut self.initialization,
        )?;
        self.emit_all(ctx.children(), writer)
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
        writer.write_all(value.format(self).map_err(Error::other)?.as_bytes())
    }
}

impl ExpressionSyntax for Uvm {
    fn parameter(&mut self, name: &str) -> String {
        format!("p_{name}")
    }
    fn literal(&mut self, value: usize) -> String {
        format!("64'd{value}")
    }
}
