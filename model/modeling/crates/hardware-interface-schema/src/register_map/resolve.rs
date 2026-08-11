use core::fmt;

use crate::register::RegisterSpec;

use super::{
    RegisterMapError, RegisterMapSpec, RepeatedRegisterBank, ResolvedRegister, ResolvedRegisterMap,
};

#[derive(Clone, Copy)]
pub(super) struct Declaration<I: 'static, F: 'static> {
    pub(super) register: RegisterSpec<I, F>,
    pub(super) index: usize,
    pub(super) bank: Option<RepeatedRegisterBank<I, F>>,
    name_scope: Option<usize>,
}

pub(super) fn resolve<I: Copy + Eq + fmt::Debug + 'static, F: Copy + Eq + fmt::Debug + 'static>(
    map: RegisterMapSpec<I, F>,
) -> Result<ResolvedRegisterMap<I, F>, RegisterMapError<I, F>> {
    let declarations = declarations(map);
    validate_duplicates(map, &declarations)?;
    let preliminary = expand(map, &declarations, false)?;
    validate_duplicate_starts(map, &preliminary)?;
    validate_definitions(map, &declarations)?;
    super::layout::validate_layout(map, &declarations)?;
    let resolved = expand(map, &declarations, true)?;
    validate_overlaps(map, &resolved)?;
    validate_paths(map, &resolved)?;
    Ok(ResolvedRegisterMap {
        root: map.name(),
        size: map.size(),
        registers: resolved,
    })
}

fn declarations<I: Copy + Eq + fmt::Debug, F: Copy + Eq + fmt::Debug>(
    map: RegisterMapSpec<I, F>,
) -> Vec<Declaration<I, F>> {
    let mut result = map
        .fixed()
        .enumerate()
        .map(|(fallback, register)| Declaration {
            register,
            index: register.declared_at(fallback),
            bank: None,
            name_scope: None,
        })
        .collect::<Vec<_>>();
    let mut fallback = result.len();
    for (bank_index, bank) in map.banks().iter().copied().enumerate() {
        for register in bank.registers().iter().copied() {
            result.push(Declaration {
                register,
                index: register.declared_at(fallback),
                bank: Some(bank),
                name_scope: Some(bank_index),
            });
            fallback += 1;
        }
    }
    result.sort_by_key(|declaration| declaration.index);
    result
}

fn validate_duplicates<I: Copy + Eq, F: Copy + Eq + fmt::Debug>(
    map: RegisterMapSpec<I, F>,
    declarations: &[Declaration<I, F>],
) -> Result<(), RegisterMapError<I, F>> {
    for (position, declaration) in declarations.iter().copied().enumerate() {
        for previous in declarations.iter().copied().take(position) {
            let numeric_match = previous.register.numeric_id().is_some()
                && previous.register.numeric_id() == declaration.register.numeric_id();
            if previous.register.id() == declaration.register.id() || numeric_match {
                return Err(RegisterMapError::DuplicatePlacementId {
                    root: map.name(),
                    path: declaration_path(map, declaration, 0),
                    declaration_index: declaration.index,
                    id: declaration.register.id(),
                });
            }
        }
    }
    for (position, declaration) in declarations.iter().copied().enumerate() {
        if declarations.iter().copied().take(position).any(|previous| {
            previous.name_scope == declaration.name_scope
                && previous.register.name() == declaration.register.name()
        }) {
            return Err(RegisterMapError::DuplicatePlacementName {
                root: map.name(),
                path: declaration_path(map, declaration, 0),
                declaration_index: declaration.index,
                name: declaration.register.name(),
            });
        }
    }
    Ok(())
}

fn validate_duplicate_starts<I: Copy, F: Copy + Eq + fmt::Debug>(
    map: RegisterMapSpec<I, F>,
    registers: &[ResolvedRegister<I, F>],
) -> Result<(), RegisterMapError<I, F>> {
    for (position, register) in registers.iter().enumerate() {
        if let Some(previous) = registers
            .iter()
            .take(position)
            .find(|previous| previous.offset == register.offset)
        {
            return Err(RegisterMapError::DuplicateExpandedStart {
                root: map.name(),
                path: register.path.clone(),
                declaration_index: register.declaration_index,
                offset: register.offset,
                first_path: previous.path.clone(),
            });
        }
    }
    Ok(())
}

fn validate_definitions<I: Copy, F: Copy + Eq + fmt::Debug>(
    map: RegisterMapSpec<I, F>,
    declarations: &[Declaration<I, F>],
) -> Result<(), RegisterMapError<I, F>> {
    for declaration in declarations.iter().copied() {
        let path = declaration_path(map, declaration, 0);
        declaration
            .register
            .definition()
            .validate_at(map.name(), &path, declaration.index)
            .map_err(RegisterMapError::Definition)?;
    }
    Ok(())
}

fn expand<I: Copy, F: Copy + Eq + fmt::Debug>(
    map: RegisterMapSpec<I, F>,
    declarations: &[Declaration<I, F>],
    overflow_is_error: bool,
) -> Result<Vec<ResolvedRegister<I, F>>, RegisterMapError<I, F>> {
    let mut registers = Vec::new();
    for declaration in declarations.iter().copied() {
        let count = declaration.bank.map_or(1, RepeatedRegisterBank::count);
        for instance in 0..count {
            let offset = declaration.bank.map_or_else(
                || Some(declaration.register.offset()),
                |bank| expanded_offset(bank, declaration.register, instance),
            );
            let Some(offset) = offset else {
                if overflow_is_error {
                    return Err(RegisterMapError::BankExpansionOverflow {
                        root: map.name(),
                        path: declaration_path(map, declaration, instance),
                        declaration_index: declaration.index,
                        instance,
                    });
                }
                continue;
            };
            registers.push(ResolvedRegister {
                path: declaration_path(map, declaration, instance),
                offset,
                declaration_index: declaration.index,
                register: declaration.register,
                bank_name: declaration.bank.map(RepeatedRegisterBank::name),
                instance: declaration.bank.map(|_| instance),
            });
        }
    }
    registers.sort_by_key(|register| (register.declaration_index, register.instance));
    Ok(registers)
}

fn validate_overlaps<I: Copy, F: Copy + Eq + fmt::Debug>(
    map: RegisterMapSpec<I, F>,
    registers: &[ResolvedRegister<I, F>],
) -> Result<(), RegisterMapError<I, F>> {
    for (position, register) in registers.iter().enumerate() {
        for previous in registers.iter().take(position) {
            if previous.offset
                < register
                    .offset
                    .saturating_add(register.definition().width())
                && register.offset
                    < previous
                        .offset
                        .saturating_add(previous.definition().width())
            {
                return Err(RegisterMapError::ExpandedOverlap {
                    root: map.name(),
                    path: register.path.clone(),
                    declaration_index: register.declaration_index,
                    first_path: previous.path.clone(),
                });
            }
        }
    }
    Ok(())
}

fn validate_paths<I: Copy, F: Copy + Eq + fmt::Debug>(
    map: RegisterMapSpec<I, F>,
    registers: &[ResolvedRegister<I, F>],
) -> Result<(), RegisterMapError<I, F>> {
    for (position, register) in registers.iter().enumerate() {
        if let Some(previous) = registers
            .iter()
            .take(position)
            .find(|previous| previous.path == register.path)
        {
            return Err(RegisterMapError::ResolvedPathCollision {
                root: map.name(),
                path: register.path.clone(),
                declaration_index: register.declaration_index,
                first_declaration_index: previous.declaration_index,
            });
        }
    }
    Ok(())
}

pub(super) const fn expanded_offset<I: Copy, F: Copy + Eq + fmt::Debug>(
    bank: RepeatedRegisterBank<I, F>,
    register: RegisterSpec<I, F>,
    instance: usize,
) -> Option<usize> {
    let Some(step) = instance.checked_mul(bank.stride()) else {
        return None;
    };
    let Some(base) = bank.base().checked_add(step) else {
        return None;
    };
    base.checked_add(register.offset())
}

pub(super) fn declaration_path<I: Copy, F: Copy + Eq + fmt::Debug>(
    map: RegisterMapSpec<I, F>,
    declaration: Declaration<I, F>,
    instance: usize,
) -> String {
    declaration.bank.map_or_else(
        || join(map.name(), declaration.register.name()),
        |bank| {
            format!(
                "{}[{}].{}",
                join(map.name(), bank.name()),
                instance,
                declaration.register.name()
            )
        },
    )
}

pub(super) fn join(root: &str, member: &str) -> String {
    if root.is_empty() {
        member.into()
    } else {
        format!("{root}.{member}")
    }
}
