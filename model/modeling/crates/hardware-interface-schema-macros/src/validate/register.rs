use std::collections::HashSet;

use crate::syntax::RegisterDecl;

pub(crate) fn check(registers: &[RegisterDecl], overlap: bool) -> syn::Result<()> {
    let mut ids = HashSet::new();
    for register in registers {
        if !ids.insert(register.id.0) {
            return Err(syn::Error::new(register.id.1, "duplicate register ID"));
        }
    }
    let mut names = HashSet::new();
    for register in registers {
        if !names.insert(register.name.value()) {
            return Err(syn::Error::new(
                register.name.span(),
                "duplicate register name",
            ));
        }
    }
    let mut ranges = Vec::new();
    for register in registers {
        validate_values(register)?;
        let end = register
            .offset
            .0
            .checked_add(register.width.0)
            .ok_or_else(|| syn::Error::new(register.offset.1, "register extent overflows usize"))?;
        if overlap
            && ranges
                .iter()
                .any(|&(start, prior_end)| start < end && register.offset.0 < prior_end)
        {
            return Err(syn::Error::new(
                register.offset.1,
                "register declarations overlap",
            ));
        }
        ranges.push((register.offset.0, end));
    }
    Ok(())
}

pub(crate) fn validate_values(register: &RegisterDecl) -> syn::Result<()> {
    if !matches!(register.width.0, 1 | 2 | 4 | 8) {
        return Err(syn::Error::new(
            register.width.1,
            "register width must be 1, 2, 4, or 8",
        ));
    }
    if !fits(register.mask.0, register.width.0) {
        return Err(syn::Error::new(
            register.mask.1,
            "writable mask does not fit register width",
        ));
    }
    if !fits(register.reset.value.0, register.width.0) {
        return Err(syn::Error::new(
            register.reset.value.1,
            "reset value does not fit register width",
        ));
    }
    Ok(())
}

const fn fits(value: u64, width: usize) -> bool {
    width == 8 || value < (1_u64 << (width * 8))
}
