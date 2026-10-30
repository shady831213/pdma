mod names;
mod register;

use super::{CHeaderExportError, CHeaderRenderer, fragment::Fragment};
use crate::export::{BankView, RegisterLocation, RegisterNode};
pub(super) use names::{MapNames, map_names};
pub(super) use register::{RegisterNames, push_register};

pub(super) fn bank_metadata(
    output: &mut Fragment,
    name: &str,
    bank: &BankView,
) -> Result<(), CHeaderExportError> {
    output.constants::<64>(
        name,
        &[
            ("BASE", bank.base),
            ("COUNT", bank.count),
            ("STRIDE", bank.stride),
        ],
    )
}

impl CHeaderRenderer {
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

    pub(super) fn bank_name(&self, index: usize) -> Result<String, CHeaderExportError> {
        let name = &self
            .map_names
            .banks
            .get(index)
            .ok_or(CHeaderExportError::InvalidVisit("bank name"))?
            .name;
        Ok(format!("{}_{name}", self.prefix))
    }

    pub(super) fn register_name(
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

    pub(super) fn register_offset(
        &self,
        node: RegisterNode<'_>,
    ) -> Result<Option<(String, usize)>, CHeaderExportError> {
        self.register_offset_name(node.location)?
            .map(|name| {
                node.view
                    .map(|view| (name, view.offset))
                    .ok_or(CHeaderExportError::InvalidVisit("register view"))
            })
            .transpose()
    }
}
