use super::super::naming::c_identifier;
use super::super::{CHeaderExportError, CRegisterBackend};
use crate::analysis::ViewPathSegment;
use crate::ast::RegisterLocation;
use crate::export::register::Context;

type Result<T> = core::result::Result<T, CHeaderExportError>;

fn path_name(segments: &[ViewPathSegment]) -> String {
    let name = segments
        .iter()
        .map(|part| {
            let name = c_identifier(&part.name);
            part.index
                .map_or_else(|| name.clone(), |index| format!("{name}_{index}"))
        })
        .collect::<Vec<_>>()
        .join("_");
    if name.len() <= 32 {
        return name;
    }
    let digest = name.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    });
    format!(
        "{}_{digest:016X}",
        name.chars().take(15).collect::<String>()
    )
}

impl CRegisterBackend {
    pub(in super::super) fn bank_name(&self, ctx: &Context<'_>) -> Result<String> {
        let bank = ctx
            .bank()
            .ok_or(CHeaderExportError::InvalidVisit("bank name"))?;
        Ok(format!("{}_{}", self.prefix, path_name(bank.segments())))
    }

    pub(in super::super) fn register_name(&self, ctx: &Context<'_>) -> Result<String> {
        let site = ctx
            .register_site()
            .ok_or(CHeaderExportError::InvalidVisit("register view"))?;
        if matches!(site.location, RegisterLocation::Root) {
            return Ok(self.prefix.clone());
        }
        Ok(format!("{}_{}", self.prefix, path_name(site.segments)))
    }

    pub(in super::super) fn register_offset(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Option<(String, usize)>> {
        let site = ctx
            .register_site()
            .ok_or(CHeaderExportError::InvalidVisit("register view"))?;
        match site.location {
            RegisterLocation::Root | RegisterLocation::Template { .. } => Ok(None),
            RegisterLocation::Standalone(_) | RegisterLocation::Instance { .. } => Ok(Some((
                format!("{}_OFFSET", self.register_name(ctx)?),
                site.offset,
            ))),
        }
    }
}
