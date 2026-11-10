use super::Uvm;
use hw_model_schema::{
    Endian,
    ast::register::Parameter,
    export::{
        Kind, SymbolScope, Visit,
        register::{self, Context},
        value::Emit,
        view,
    },
};
use std::io::{Error, Write};
impl register::Backend for Uvm {
    fn register<'a>(
        &mut self,
        reg: view::Register<'a>,
        ctx: Context<'a>,
        writer: &mut dyn Write,
    ) -> Result<(), Error> {
        if reg.endian() != Endian::Little {
            return Err(Error::other(
                "UVM backend currently requires little-endian registers",
            ));
        }
        self.variants = SymbolScope::default();
        self.initialization.clear();
        let name = ctx.entry_name();
        writeln!(
            writer,
            "  class {name} extends uvm_reg;\n    `uvm_object_utils({name})"
        )?;
        self.emit_all(ctx.children(), writer)?;
        if self.initialization.is_empty() {
            writeln!(writer, "    uvm_reg_field value_field;")?;
            super::fields::ral_field(reg, ("value", 0, reg.width() * 8), &mut self.initialization)?;
        }
        writeln!(
            writer,
            "    function new(string name = \"{name}\");\n      super.new(name, {}, UVM_NO_COVERAGE);\n    endfunction",
            reg.width() * 8
        )?;
        writeln!(writer, "    virtual function void build();")?;
        writer.write_all(&self.initialization)?;
        writeln!(writer, "    endfunction\n  endclass\n")
    }

    fn register_map<'a>(
        &mut self,
        map: view::Map<'a>,
        ctx: Context<'a>,
        writer: &mut dyn Write,
    ) -> Result<(), Error> {
        if map.is_projection() {
            return Err(Error::other(
                "permission-projected register maps are unsupported",
            ));
        }
        self.initialization.clear();
        let name = ctx.entry_name();
        writeln!(
            writer,
            "  class {name} extends uvm_reg_block;\n    `uvm_object_utils({name})"
        )?;
        let (parameters, members): (Vec<_>, Vec<_>) = ctx
            .children()
            .partition(|visit| visit.kind() == Kind::Parameter);
        let mut parameter_text = Vec::new();
        self.emit_all(parameters, &mut parameter_text)?;
        self.emit_all(
            members
                .into_iter()
                .filter(|visit| visit.kind() != Kind::LayoutProperty),
            writer,
        )?;
        writeln!(
            writer,
            "    function new(string name = \"{name}\");\n      super.new(name, UVM_NO_COVERAGE);\n    endfunction"
        )?;
        write!(writer, "    virtual function void build(")?;
        writer.write_all(&parameter_text)?;
        writeln!(writer, "input int unsigned bus_bytes = 4);")?;
        writeln!(
            writer,
            "      default_map = create_map(\"default_map\", 0, bus_bytes, UVM_LITTLE_ENDIAN, 1);"
        )?;
        writer.write_all(&self.initialization)?;
        writeln!(writer, "    endfunction\n  endclass\n")
    }

    fn reference<'a>(
        &mut self,
        _: view::Reference<'a>,
        target: view::Target<'a>,
        visit: Visit<Context<'a>>,
        ctx: Context<'a>,
        writer: &mut dyn Write,
    ) -> Result<(), Error> {
        if matches!(target.kind, Kind::Bits | Kind::Enum) {
            return self.emit(visit, writer);
        }
        let p = ctx
            .placement()
            .ok_or_else(|| Error::other("reference without placement"))?;
        let member = super::fields::member(p.name(), target.kind)?;
        let array = if p.is_bank() { "[]" } else { "" };
        writeln!(writer, "    {} {member}{array};", target.name)?;
        let mut initialization = std::mem::take(&mut self.initialization);
        self.initialize_reference(target, &ctx, &mut initialization)?;
        self.initialization = initialization;
        Ok(())
    }
    fn placement<'a>(
        &mut self,
        _: view::Placement<'a>,
        ctx: Context<'a>,
        writer: &mut dyn Write,
    ) -> Result<(), Error> {
        self.emit_all(
            ctx.children().filter(|v| v.kind() == Kind::Reference),
            writer,
        )
    }

    fn parameter(
        &mut self,
        value: Parameter,
        _: Context<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Error> {
        if value.binding.is_none() {
            write!(writer, "input int unsigned p_{}, ", value.name)?;
        }
        Ok(())
    }

    fn argument<'a>(
        &mut self,
        _: view::Argument<'a>,
        ctx: Context<'a>,
        writer: &mut dyn Write,
    ) -> Result<(), Error> {
        self.emit_all(ctx.children(), writer)?;
        writer.write_all(b", ")
    }

    fn layout_property<'a>(
        &mut self,
        _: register::LayoutProperty<'a>,
        ctx: Context<'a>,
        writer: &mut dyn Write,
    ) -> Result<(), Error> {
        self.emit_all(ctx.children(), writer)
    }
}
