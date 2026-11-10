use super::UvmRegisterBackend;
use crate::export::Context as _;
use crate::{
    EnumVariantSpec,
    export::{
        expression::ExpressionSyntax,
        register::{self, Context},
        value::{self, DocumentationOwner, Emit},
        view,
    },
};
use std::io::{Error, Write};

impl value::Backend for UvmRegisterBackend {
    type Error = Error;
    type Domain = register::Domain;
    type Context<'a> = Context<'a>;

    fn check_entry(
        &mut self,
        ctx: &Context<'_>,
        symbols: &mut crate::export::EntrySymbols,
    ) -> Result<(), Error> {
        if matches!(
            ctx.entry_kind(),
            crate::export::Kind::Register | crate::export::Kind::RegisterMap
        ) {
            let name = self.names.get(ctx.entry_id(), ctx.entry_name());
            if name.starts_with("uvm_") || name == "type_id" {
                return Err(Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "class name conflicts with the UVM macro namespace; select an alias",
                ));
            }
            self.names.check(ctx.entry_id(), ctx.entry_name(), symbols)
        } else {
            Ok(())
        }
    }

    fn bit_field(
        &mut self,
        field: view::BitField,
        ctx: Context<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Error> {
        self.symbols
            .declare(&format!("{}_field", field.name()), ctx.entry_id())?;
        writeln!(writer, "    uvm_pkg::uvm_reg_field {}_field;", field.name())?;
        let reg = ctx
            .register()
            .ok_or_else(|| Error::other("missing register owner"))?;
        super::fields::ral_field(
            reg,
            (field.name(), field.lsb(), field.bit_width()),
            self.volatile,
            self.compare,
            &mut self.build_body,
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
        self.symbols.declare(&name, ctx.entry_id())?;
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
        super::super::sv::comment(text, writer)
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

impl ExpressionSyntax for UvmRegisterBackend {
    fn parameter(&mut self, name: &str) -> String {
        format!("p_{name}")
    }
    fn literal(&mut self, value: usize) -> String {
        format!("64'd{value}")
    }
}
