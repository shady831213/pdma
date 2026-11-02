use super::super::naming::{c_identifier, disambiguate_identifiers};
use super::super::{CHeaderExportError, CRegisterBackend};
use crate::analysis::RegisterMapView;
use crate::ast::RegisterLocation;
use crate::export::register::EmitContext;

pub(in super::super) fn map_names(schema: &RegisterMapView) -> MapNames {
    let bank_names = disambiguate_identifiers(
        schema
            .banks
            .iter()
            .enumerate()
            .map(|(index, bank)| {
                let name = c_identifier(bank.name);
                if schema.expanded_banks && name.len() > 32 {
                    format!("BANK_{index}")
                } else {
                    name
                }
            })
            .collect(),
    );
    let mut bases = schema
        .registers
        .iter()
        .map(|register| c_identifier(register.name))
        .collect::<Vec<_>>();
    for (bank, bank_name) in schema.banks.iter().zip(&bank_names) {
        bases.extend(
            bank.registers
                .iter()
                .map(|register| format!("{bank_name}_{}", c_identifier(register.name))),
        );
    }
    let fixed_count = schema.registers.len();
    let mut bases = disambiguate_identifiers(bases).into_iter();
    let fixed: Vec<String> = bases.by_ref().take(fixed_count).collect();
    let mut banks = schema
        .banks
        .iter()
        .zip(bank_names)
        .map(|(bank, name)| {
            let bank_prefix = format!("{name}_");
            let members = bases
                .by_ref()
                .take(bank.registers.len())
                .map(|member| {
                    member
                        .strip_prefix(&bank_prefix)
                        .map_or_else(|| member.clone(), String::from)
                })
                .collect();
            BankNames {
                name,
                members,
                offsets: Vec::new(),
            }
        })
        .collect::<Vec<_>>();
    let mut offsets: Vec<String> = fixed.iter().map(|name| format!("{name}_OFFSET")).collect();
    for (bank, names) in schema.banks.iter().zip(&banks) {
        for instance in 0..bank.count {
            offsets.extend(
                names
                    .members
                    .iter()
                    .map(|member| format!("{}_{}_{}_OFFSET", names.name, instance, member)),
            );
        }
    }
    let mut offsets = disambiguate_identifiers(offsets).into_iter();
    let fixed_offsets = offsets.by_ref().take(fixed_count).collect();
    for (bank, names) in schema.banks.iter().zip(&mut banks) {
        names.offsets = (0..bank.count)
            .map(|_| offsets.by_ref().take(bank.registers.len()).collect())
            .collect();
    }
    MapNames {
        fixed,
        fixed_offsets,
        banks,
    }
}

#[derive(Debug, Default)]
pub(in super::super) struct MapNames {
    pub(super) fixed: Vec<String>,
    pub(super) fixed_offsets: Vec<String>,
    pub(super) banks: Vec<BankNames>,
}

#[derive(Debug)]
pub(super) struct BankNames {
    pub(super) name: String,
    pub(super) members: Vec<String>,
    pub(super) offsets: Vec<Vec<String>>,
}

impl CRegisterBackend {
    pub(in super::super) fn bank_name(&self, index: usize) -> Result<String, CHeaderExportError> {
        let name = &self
            .map_names
            .banks
            .get(index)
            .ok_or(CHeaderExportError::InvalidVisit("bank name"))?
            .name;
        Ok(format!("{}_{name}", self.prefix))
    }

    pub(in super::super) fn register_name(
        &self,
        location: RegisterLocation,
    ) -> Result<String, CHeaderExportError> {
        match location {
            RegisterLocation::Root => Ok(self.prefix.clone()),
            RegisterLocation::Standalone(index) => {
                let name = self
                    .map_names
                    .fixed
                    .get(index)
                    .ok_or(CHeaderExportError::InvalidVisit("register name"))?;
                Ok(format!("{}_{name}", self.prefix))
            }
            RegisterLocation::Template { bank, member }
            | RegisterLocation::Instance { bank, member, .. } => {
                let bank = self
                    .map_names
                    .banks
                    .get(bank)
                    .ok_or(CHeaderExportError::InvalidVisit("bank name"))?;
                let member = bank
                    .members
                    .get(member)
                    .ok_or(CHeaderExportError::InvalidVisit("bank member name"))?;
                Ok(format!("{}_{}_{member}", self.prefix, bank.name))
            }
        }
    }

    fn register_offset_name(
        &self,
        location: RegisterLocation,
    ) -> Result<Option<String>, CHeaderExportError> {
        let name = match location {
            RegisterLocation::Root | RegisterLocation::Template { .. } => return Ok(None),
            RegisterLocation::Standalone(index) => self.map_names.fixed_offsets.get(index),
            RegisterLocation::Instance {
                bank,
                instance,
                member,
            } => self
                .map_names
                .banks
                .get(bank)
                .and_then(|bank| bank.offsets.get(instance))
                .and_then(|names| names.get(member)),
        }
        .ok_or(CHeaderExportError::InvalidVisit("register offset name"))?;
        Ok(Some(format!("{}_{name}", self.prefix)))
    }

    pub(in super::super) fn register_offset(
        &self,
        ctx: &EmitContext<'_>,
    ) -> Result<Option<(String, usize)>, CHeaderExportError> {
        self.register_offset_name(ctx.register_location())?
            .map(|name| {
                ctx.register_site()
                    .map(|site| (name, site.offset))
                    .ok_or(CHeaderExportError::InvalidVisit("register view"))
            })
            .transpose()
    }
}
