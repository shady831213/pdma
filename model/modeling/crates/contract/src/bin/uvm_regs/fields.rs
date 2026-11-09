use hw_model_schema::{Access, Endian, ResetKind, ast::RegisterInfo};
use std::io::{Error, Write};

pub(super) const fn access(value: Access) -> &'static str {
    match value {
        Access::ReadOnly => "RO",
        Access::ReadWrite => "RW",
        Access::WriteOnly => "WO",
    }
}

struct Field<'a> {
    name: &'a str,
    lsb: usize,
    width: usize,
    access: &'static str,
    reset: u64,
    known_reset: bool,
}

fn describe<'a>(reg: &RegisterInfo, shape: (&'a str, usize, usize)) -> Result<Field<'a>, Error> {
    let (name, lsb, width) = shape;
    let mask = u64::MAX >> (64 - width);
    let writable = (reg.writable_mask() >> lsb) & mask;
    if writable != 0 && writable != mask {
        return Err(Error::other(format!(
            "{}.{name}: partial writable field mask is unsupported",
            reg.name()
        )));
    }
    let rights = match (reg.access(), writable != 0) {
        (Access::ReadOnly, _) | (Access::ReadWrite, false) => "RO",
        (Access::ReadWrite, true) => "RW",
        (Access::WriteOnly, true) => "WO",
        (Access::WriteOnly, false) => {
            return Err(Error::other("write-only field without writable bits"));
        }
    };
    Ok(Field {
        name,
        lsb,
        width,
        access: rights,
        reset: (reg.reset_value() >> lsb) & mask,
        known_reset: match reg.reset_kind() {
            ResetKind::Constant | ResetKind::Zero => true,
            ResetKind::PostReset => false,
        },
    })
}

pub(super) fn emit(reg: &RegisterInfo, writer: &mut dyn Write) -> Result<(), Error> {
    if reg.endian() != Endian::Little {
        return Err(Error::other(
            "UVM backend currently requires little-endian registers",
        ));
    }
    let fields = if reg.fields().is_empty() {
        vec![describe(reg, ("value", 0, reg.width() * 8))?]
    } else {
        reg.fields()
            .iter()
            .map(|f| describe(reg, (f.name(), f.lsb(), f.bit_width())))
            .collect::<Result<Vec<_>, _>>()?
    };
    let name = reg.name();
    writeln!(writer, "  class {name} extends uvm_reg;")?;
    writeln!(writer, "    `uvm_object_utils({name})")?;
    for field in &fields {
        writeln!(writer, "    uvm_reg_field {}_field;", field.name)?;
    }
    writeln!(
        writer,
        "    function new(string name = \"{name}\");\n      super.new(name, {}, UVM_NO_COVERAGE);\n    endfunction",
        reg.width() * 8
    )?;
    writeln!(writer, "    virtual function void build();")?;
    for f in fields {
        writeln!(
            writer,
            "      {0}_field = uvm_reg_field::type_id::create(\"{0}\");",
            f.name
        )?;
        writeln!(
            writer,
            "      {}_field.configure(this, {}, {}, \"{}\", 1, 64'h{:x}, {}, 0, 0);",
            f.name,
            f.width,
            f.lsb,
            f.access,
            f.reset,
            u8::from(f.known_reset)
        )?;
        writeln!(writer, "      {}_field.set_compare(UVM_NO_CHECK);", f.name)?;
    }
    writeln!(writer, "    endfunction\n  endclass\n")
}
