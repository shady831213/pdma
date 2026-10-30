use super::super::{CHeaderExportError, fragment::Fragment};
use crate::{Access, ResetKind, graph::RegisterInfo};

#[derive(Clone, Copy)]
pub(in super::super) struct RegisterNames<'a> {
    pub(in super::super) prefix: &'a str,
    pub(in super::super) name: &'a str,
    pub(in super::super) offset: Option<(&'a str, usize)>,
}

pub(in super::super) fn push_register(
    output: &mut Fragment,
    names: RegisterNames<'_>,
    register: &RegisterInfo,
) -> Result<(), CHeaderExportError> {
    let RegisterNames {
        prefix,
        name,
        offset,
    } = names;
    if let Some((offset_name, offset)) = offset {
        output.define(offset_name, format_args!("UINT64_C({offset})"))?;
    }
    output.define(
        &format!("{name}_WIDTH"),
        format_args!("UINT64_C({})", register.width()),
    )?;
    let access = match register.access() {
        Access::ReadOnly => "READ_ONLY",
        Access::ReadWrite => "READ_WRITE",
        Access::WriteOnly => "WRITE_ONLY",
    };
    let reset = match register.reset_kind() {
        ResetKind::Constant => "CONSTANT",
        ResetKind::Zero => "ZERO",
        ResetKind::PostReset => "POST_RESET",
    };
    output.define(
        &format!("{name}_ACCESS"),
        format_args!("{prefix}_ACCESS_{access}"),
    )?;
    output.define(
        &format!("{name}_RESET_KIND"),
        format_args!("{prefix}_RESET_{reset}"),
    )?;
    output.define(
        &format!("{name}_RESET_VALUE"),
        format_args!("UINT64_C({})", register.reset_value()),
    )?;
    output.define(
        &format!("{name}_WRITABLE_MASK"),
        format_args!("UINT64_C({})", register.writable_mask()),
    )?;
    Ok(())
}
