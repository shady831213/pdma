use hw_model_schema::{
    analysis::CompleteMapLayout,
    ast::{DefinitionBody, LayoutExpr},
    export::expression::{ExpressionSyntax, format_expression},
};
use std::io::{Error, Write};

struct Syntax;
impl ExpressionSyntax for Syntax {
    fn parameter(&mut self, name: &str) -> String {
        format!("p_{name}")
    }
    fn literal(&mut self, value: usize) -> String {
        format!("64'd{value}")
    }
}
fn expression(value: &LayoutExpr) -> Result<String, Error> {
    format_expression(value, &mut Syntax).map_err(Error::other)
}

pub(super) fn emit(
    map: &CompleteMapLayout,
    name: &str,
    writer: &mut dyn Write,
) -> Result<(), Error> {
    writeln!(
        writer,
        "  class {name} extends uvm_reg_block;\n    `uvm_object_utils({name})"
    )?;
    for p in map.placements() {
        let suffix = match p.reference.target().body() {
            DefinitionBody::Register(_) => "reg",
            DefinitionBody::RegisterMap(_) => "block",
            _ => return Err(Error::other("unsupported placement")),
        };
        let array = if p.bank { "[]" } else { "" };
        writeln!(
            writer,
            "    {} {}_{suffix}{array};",
            p.child_name,
            p.reference.name()
        )?;
    }
    writeln!(
        writer,
        "    function new(string name = \"{name}\");\n      super.new(name, UVM_NO_COVERAGE);\n    endfunction"
    )?;
    write!(writer, "    virtual function void build(")?;
    for parameter in map.parameters() {
        write!(writer, "input int unsigned p_{parameter}, ")?;
    }
    writeln!(writer, "input int unsigned bus_bytes = 4);")?;
    writeln!(
        writer,
        "      default_map = create_map(\"default_map\", 0, bus_bytes, UVM_LITTLE_ENDIAN, 1);"
    )?;
    for p in map.placements() {
        let target = p.reference.target();
        let register = match target.body() {
            DefinitionBody::Register(r) => Some(r),
            DefinitionBody::RegisterMap(_) => None,
            _ => return Err(Error::other("unsupported placement")),
        };
        let suffix = if register.is_some() { "reg" } else { "block" };
        let member = format!("{}_{suffix}", p.reference.name());
        let mut handle = member.clone();
        let mut offset = expression(&p.offset)?;
        let instance_name = if p.bank {
            writeln!(
                writer,
                "      if ({} == 0) `uvm_fatal(\"RAL_BANK_COUNT\", \"{member} count must be positive\")",
                expression(&p.count)?
            )?;
            writeln!(
                writer,
                "      {member} = new[{}];\n      foreach ({member}[i]) begin",
                expression(&p.count)?
            )?;
            handle.push_str("[i]");
            offset = format!("({offset} + i * {})", expression(&p.stride)?);
            format!("$sformatf(\"{member}[%0d]\", i)")
        } else {
            format!("\"{member}\"")
        };
        writeln!(
            writer,
            "      {handle} = {}::type_id::create({instance_name});\n      {handle}.configure(this);",
            p.child_name
        )?;
        if let Some(reg) = register {
            writeln!(
                writer,
                "      {handle}.build();\n      default_map.add_reg({handle}, {offset}, \"{}\");",
                super::fields::access(reg.access())
            )?;
        } else {
            write!(writer, "      {handle}.build(")?;
            for argument in &p.arguments {
                write!(writer, "{}, ", expression(argument)?)?;
            }
            writeln!(
                writer,
                "bus_bytes);\n      default_map.add_submap({handle}.default_map, {offset});"
            )?;
        }
        if p.bank {
            writeln!(writer, "      end")?;
        }
    }
    writeln!(writer, "    endfunction\n  endclass\n")
}
