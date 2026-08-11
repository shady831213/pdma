use core::fmt;

use super::resolve::{Declaration, declaration_path, expanded_offset, join};
use super::{RegisterMapError, RegisterMapSpec, RepeatedRegisterBank};

pub(super) fn validate_layout<I: Copy + Eq + fmt::Debug, F: Copy + Eq + fmt::Debug>(
    map: RegisterMapSpec<I, F>,
    declarations: &[Declaration<I, F>],
) -> Result<(), RegisterMapError<I, F>> {
    for declaration in declarations
        .iter()
        .copied()
        .filter(|item| item.bank.is_none())
    {
        check_extent(map, declaration, 0, declaration.register.offset())?;
    }
    let mut declaration_base = map.fixed().count();
    for bank in map.banks().iter().copied() {
        let index = bank
            .registers()
            .first()
            .map_or(declaration_base, |register| {
                register.declared_at(declaration_base)
            });
        let path = join(map.name(), bank.name());
        validate_stride(map, bank, index, path)?;
        validate_bank_instances(map, bank, declarations)?;
        declaration_base += bank.registers().len();
    }
    Ok(())
}

fn validate_stride<I: Copy, F: Copy + Eq + fmt::Debug>(
    map: RegisterMapSpec<I, F>,
    bank: RepeatedRegisterBank<I, F>,
    declaration_index: usize,
    path: String,
) -> Result<(), RegisterMapError<I, F>> {
    if bank.stride() == 0 {
        return Err(RegisterMapError::BankStrideZero {
            root: map.name(),
            path,
            declaration_index,
        });
    }
    let alignment = bank
        .registers()
        .iter()
        .fold(1, |widest, register| widest.max(register.width()));
    if bank.stride() % alignment != 0 {
        return Err(RegisterMapError::BankStrideMisaligned {
            root: map.name(),
            path,
            declaration_index,
            stride: bank.stride(),
            required_alignment: alignment,
        });
    }
    let extent = bank.registers().iter().fold(0, |largest, register| {
        register
            .offset()
            .checked_add(register.width())
            .map_or(usize::MAX, |end| largest.max(end))
    });
    if bank.stride() < extent {
        return Err(RegisterMapError::BankStrideTooSmall {
            root: map.name(),
            path,
            declaration_index,
            stride: bank.stride(),
            required_extent: extent,
        });
    }
    Ok(())
}

fn validate_bank_instances<I: Copy + Eq + fmt::Debug, F: Copy + Eq + fmt::Debug>(
    map: RegisterMapSpec<I, F>,
    bank: RepeatedRegisterBank<I, F>,
    declarations: &[Declaration<I, F>],
) -> Result<(), RegisterMapError<I, F>> {
    for instance in 0..bank.count() {
        for declaration in declarations
            .iter()
            .copied()
            .filter(|item| item.bank == Some(bank))
        {
            let offset =
                expanded_offset(bank, declaration.register, instance).ok_or_else(|| {
                    RegisterMapError::BankExpansionOverflow {
                        root: map.name(),
                        path: declaration_path(map, declaration, instance),
                        declaration_index: declaration.index,
                        instance,
                    }
                })?;
            check_extent(map, declaration, instance, offset)?;
        }
    }
    Ok(())
}

fn check_extent<I: Copy, F: Copy + Eq + fmt::Debug>(
    map: RegisterMapSpec<I, F>,
    declaration: Declaration<I, F>,
    instance: usize,
    offset: usize,
) -> Result<(), RegisterMapError<I, F>> {
    if offset
        .checked_add(declaration.register.width())
        .is_none_or(|end| end > map.size())
    {
        return Err(RegisterMapError::RegisterOutOfBounds {
            root: map.name(),
            path: declaration_path(map, declaration, instance),
            declaration_index: declaration.index,
            offset,
            width: declaration.register.width(),
            map_size: map.size(),
        });
    }
    Ok(())
}
