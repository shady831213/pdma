use super::{ResolvedRegisterBank, ResolvedRegisterMap};
use crate::RegisterSpec;

/// One owned link from a resolved leaf to its enclosing bank declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisterBankInstance {
    path: String,
    index: usize,
}
impl RegisterBankInstance {
    /// Returns the enclosing bank declaration's root-qualified path.
    pub fn path(&self) -> &str {
        &self.path
    }
    /// Returns the selected bank instance.
    pub const fn index(&self) -> usize {
        self.index
    }
}

impl<I: Copy, F: Copy + Eq + core::fmt::Debug> ResolvedRegisterMap<I, F> {
    pub(super) fn collect_banks(&mut self) {
        if !self
            .placements
            .iter()
            .any(|node| node.placement().is_bank())
        {
            return;
        }
        self.banks = self
            .placements
            .iter()
            .filter(|node| node.placement().is_bank())
            .map(|node| {
                let placement = node.placement();
                let prefix = format!("{}[", node.path());
                let first = format!("{}[0]", node.path());
                let expanded_registers: Vec<_> = self
                    .registers
                    .iter()
                    .filter(|leaf| leaf.path().starts_with(&prefix))
                    .cloned()
                    .collect();
                let registers = expanded_registers
                    .iter()
                    .filter(|leaf| {
                        leaf.path() == first
                            || leaf
                                .path()
                                .strip_prefix(&first)
                                .is_some_and(|tail| tail.starts_with('.'))
                    })
                    .map(|leaf| {
                        RegisterSpec::place(
                            leaf.id(),
                            leaf.name(),
                            leaf.offset() - placement.offset(),
                            leaf.definition(),
                        )
                        .with_documentation(leaf.documentation())
                    })
                    .collect();
                ResolvedRegisterBank {
                    ordered: true,
                    documentation: placement.documentation(),
                    path: node.path().to_owned(),
                    name: placement.name(),
                    declaration_index: expanded_registers
                        .first()
                        .map_or(0, super::ResolvedRegister::declaration_index),
                    base: placement.offset(),
                    count: placement.count(),
                    stride: placement.stride(),
                    registers,
                    expanded_registers,
                }
            })
            .collect();
        for leaf in &mut self.registers {
            leaf.ancestors = self
                .banks
                .iter()
                .filter_map(|bank| {
                    let suffix = leaf.path().strip_prefix(bank.path())?.strip_prefix('[')?;
                    let (index, _) = suffix.split_once(']')?;
                    Some(RegisterBankInstance {
                        path: bank.path().to_owned(),
                        index: index.parse().ok()?,
                    })
                })
                .collect();
        }
        for bank in &mut self.banks {
            for leaf in &mut bank.expanded_registers {
                if let Some(source) = self
                    .registers
                    .iter()
                    .find(|source| source.path() == leaf.path())
                {
                    leaf.ancestors.clone_from(&source.ancestors);
                }
            }
        }
    }
}
