use super::UvmRegisterBackend;
use crate::export::Context as _;
use crate::{
    Endian,
    ast::register::Parameter,
    export::{
        Kind, Visit,
        register::{self, Context},
        value::Emit,
        view,
    },
};
use std::io::{Error, Write};
impl register::Backend for UvmRegisterBackend {
    fn entry(&mut self, ctx: Context<'_>, w: &mut dyn Write) -> Result<(), Error> {
        if matches!(ctx.entry_kind(), Kind::Register | Kind::RegisterMap) {
            self.emit(ctx.current(), w)
        } else {
            Ok(())
        }
    }

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
        self.build_body.clear();
        self.symbols = super::super::sv::Symbols::default();
        let name = self.names.get(ctx.entry_id(), ctx.entry_name());
        writeln!(
            writer,
            "  class {name} extends uvm_pkg::uvm_reg;\n    `uvm_object_utils({name})"
        )?;
        self.emit_all(ctx.children(), writer)?;
        if self.build_body.is_empty() {
            writeln!(writer, "    uvm_pkg::uvm_reg_field value_field;")?;
            super::fields::ral_field(
                reg,
                ("value", 0, reg.width() * 8),
                self.volatile,
                self.compare,
                &mut self.build_body,
            )?;
        }
        writeln!(
            writer,
            "    function new(string name = \"{name}\");\n      super.new(name, {}, uvm_pkg::UVM_NO_COVERAGE);\n    endfunction",
            reg.width() * 8
        )?;
        writeln!(writer, "    virtual function void build();")?;
        writer.write_all(&self.build_body)?;
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
        self.build_body.clear();
        self.symbols = super::super::sv::Symbols::default();
        let name = self.names.get(ctx.entry_id(), ctx.entry_name());
        writeln!(
            writer,
            "  class {name} extends uvm_pkg::uvm_reg_block;\n    `uvm_object_utils({name})"
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
            "    function new(string name = \"{name}\");\n      super.new(name, uvm_pkg::UVM_NO_COVERAGE);\n    endfunction"
        )?;
        write!(writer, "    virtual function void build(")?;
        writer.write_all(&parameter_text)?;
        writeln!(writer, "input int unsigned bus_bytes = 4);")?;
        writeln!(
            writer,
            "      default_map = create_map(\"default_map\", 0, bus_bytes, uvm_pkg::UVM_LITTLE_ENDIAN, 1);"
        )?;
        writer.write_all(&self.build_body)?;
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
        let suffix = match target.kind {
            Kind::Register => "reg",
            Kind::RegisterMap => "block",
            _ => return Err(Error::other("unsupported placement")),
        };
        let member = format!("{}_{suffix}", p.name());
        let array = if p.is_bank() { "[]" } else { "" };
        self.symbols.declare(&member, ctx.entry_id())?;
        let target_name = self.names.get(target.id, target.name);
        // Parameters may shadow members accessed through `this`, but never class types.
        if target_name == "bus_bytes"
            || target_name
                .strip_prefix("p_")
                .is_some_and(|name| ctx.parameters().contains(&name))
        {
            return Err(Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("class {target_name} conflicts with a build argument; select an alias"),
            ));
        }
        self.symbols.reference_type(&target_name, target.id)?;
        writeln!(writer, "    {target_name} {member}{array};")?;
        let mut body = std::mem::take(&mut self.build_body);
        let result = self.emit_member_build(&member, target, &ctx, &mut body);
        self.build_body = body;
        result?;
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
            super::super::sv::valid_name(&format!("p_{}", value.name))?;
            write!(writer, "input longint unsigned p_{}, ", value.name)?;
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
}
