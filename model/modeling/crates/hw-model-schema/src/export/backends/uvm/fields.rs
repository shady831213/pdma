use crate::{Access, ResetKind, export::view::Register};
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

fn describe<'a>(reg: Register<'_>, shape: (&'a str, usize, usize)) -> Result<Field<'a>, Error> {
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

pub(super) fn ral_field(
    reg: Register<'_>,
    shape: (&str, usize, usize),
    volatile: bool,
    compare: bool,
    writer: &mut dyn Write,
) -> Result<(), Error> {
    let f = describe(reg, shape)?;
    writeln!(
        writer,
        "      {0}_field = uvm_pkg::uvm_reg_field::type_id::create(\"{0}\");",
        f.name
    )?;
    writeln!(
        writer,
        "      {}_field.configure(this, {}, {}, \"{}\", {}, 64'h{:x}, {}, 0, 0);",
        f.name,
        f.width,
        f.lsb,
        f.access,
        u8::from(volatile),
        f.reset,
        u8::from(f.known_reset)
    )?;
    writeln!(
        writer,
        "      {}_field.set_compare({});",
        f.name,
        if compare {
            "uvm_pkg::UVM_CHECK"
        } else {
            "uvm_pkg::UVM_NO_CHECK"
        }
    )
}
