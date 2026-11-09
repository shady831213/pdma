use super::backend::Uvm;
use hw_model_schema::{
    Access, ResetKind,
    export::{Kind, register::Context, value::Emit, view::Register},
};
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

pub(super) fn member(name: &str, kind: Kind) -> Result<String, Error> {
    let suffix = match kind {
        Kind::Register => "reg",
        Kind::RegisterMap => "block",
        _ => return Err(Error::other("unsupported placement")),
    };
    Ok(format!("{name}_{suffix}"))
}

pub(super) fn ral_field(
    reg: Register<'_>,
    shape: (&str, usize, usize),
    writer: &mut dyn Write,
) -> Result<(), Error> {
    let f = describe(reg, shape)?;
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
    writeln!(writer, "      {}_field.set_compare(UVM_NO_CHECK);", f.name)
}

impl Uvm {
    pub(super) fn field(
        &mut self,
        shape: (&str, usize, usize),
        ctx: &Context<'_>,
        writer: &mut dyn Write,
    ) -> Result<(), Error> {
        writeln!(writer, "    uvm_reg_field {}_field;", shape.0)?;
        let reg = ctx
            .register()
            .ok_or_else(|| Error::other("missing register owner"))?;
        ral_field(reg, shape, &mut self.initialization)?;
        self.emit_all(ctx.children(), writer)
    }
}
