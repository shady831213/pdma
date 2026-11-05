use super::super::{CHeaderExportError, CRegisterBackend, fragment::Fragment};
use crate::{Access, ResetKind};

pub(in super::super) fn register_access(
    output: &mut Fragment,
    prefix: &str,
    name: &str,
    access: Access,
) -> Result<(), CHeaderExportError> {
    let access = match access {
        Access::ReadOnly => "READ_ONLY",
        Access::ReadWrite => "READ_WRITE",
        Access::WriteOnly => "WRITE_ONLY",
    };
    output.define(
        &format!("{name}_ACCESS"),
        format_args!("{prefix}_ACCESS_{access}"),
    )
}

pub(in super::super) fn register_reset(
    output: &mut Fragment,
    prefix: &str,
    name: &str,
    kind: ResetKind,
    value: u64,
) -> Result<(), CHeaderExportError> {
    let kind = match kind {
        ResetKind::Constant => "CONSTANT",
        ResetKind::Zero => "ZERO",
        ResetKind::PostReset => "POST_RESET",
    };
    output.define(
        &format!("{name}_RESET_KIND"),
        format_args!("{prefix}_RESET_{kind}"),
    )?;
    output.unsigned::<64>(&format!("{name}_RESET_VALUE"), value)
}

impl CRegisterBackend {
    pub(super) fn map_constants(&mut self, size: usize) -> Result<(), CHeaderExportError> {
        self.output
            .constants::<64>(&self.prefix, &[("SIZE", size)])?;
        self.output.constants::<32>(
            &self.prefix,
            &[
                ("ACCESS_READ_ONLY", 0),
                ("ACCESS_READ_WRITE", 1),
                ("ACCESS_WRITE_ONLY", 2),
                ("RESET_CONSTANT", 0),
                ("RESET_ZERO", 1),
                ("RESET_POST_RESET", 2),
            ],
        )
    }
}
