use core::fmt;

use crate::register::RegisterSpec;

use super::Declaration;
use crate::register_map::{
    RegisterMapError, RegisterMapSpec, RepeatedRegisterBank, ResolvedRegister, ResolvedRegisterBank,
};

pub(in crate::register_map) fn resolved_banks<
    I: Copy + Eq + fmt::Debug,
    F: Copy + Eq + fmt::Debug,
>(
    map: RegisterMapSpec<I, F>,
    declarations: &[Declaration<I, F>],
    expanded: &[ResolvedRegister<I, F>],
) -> Vec<ResolvedRegisterBank<I, F>> {
    map.banks()
        .iter()
        .copied()
        .map(|bank| {
            let mut members = declarations
                .iter()
                .copied()
                .filter(|declaration| declaration.bank == Some(bank))
                .collect::<Vec<_>>();
            members.sort_by_key(|member| member.index);
            ResolvedRegisterBank {
                path: join(map.name(), bank.name()),
                name: bank.name(),
                declaration_index: members.first().map_or(0, |member| member.index),
                base: bank.base(),
                count: bank.count(),
                stride: bank.stride(),
                registers: members.into_iter().map(|member| member.register).collect(),
                expanded_registers: expanded
                    .iter()
                    .filter(|register| register.bank_name() == Some(bank.name()))
                    .cloned()
                    .collect(),
            }
        })
        .collect()
}

pub(in crate::register_map) fn expand<I: Copy, F: Copy + Eq + fmt::Debug>(
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

pub(in crate::register_map) const fn expanded_offset<I: Copy, F: Copy + Eq + fmt::Debug>(
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

pub(in crate::register_map) fn declaration_path<I: Copy, F: Copy + Eq + fmt::Debug>(
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

pub(in crate::register_map) fn join(root: &str, member: &str) -> String {
    if root.is_empty() {
        member.into()
    } else {
        format!("{root}.{member}")
    }
}
