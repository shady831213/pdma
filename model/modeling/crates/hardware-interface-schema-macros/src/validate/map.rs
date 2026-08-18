use std::collections::HashSet;

use crate::internal::{
    syntax::{MapDecl, RegisterDecl},
    validate::register::validate_values,
};

pub(in crate::internal) fn check(decl: &MapDecl) -> syn::Result<()> {
    let declarations: Vec<&RegisterDecl> = decl
        .fixed
        .iter()
        .chain(decl.banks.iter().flat_map(|bank| &bank.registers))
        .collect();
    let mut ids = HashSet::new();
    for register in &declarations {
        if !ids.insert(register.id.0) {
            return Err(syn::Error::new(register.id.1, "duplicate register ID"));
        }
    }
    let mut names = HashSet::new();
    for register in &declarations {
        if !names.insert(register.name.value()) {
            return Err(syn::Error::new(
                register.name.span(),
                "duplicate register name",
            ));
        }
    }
    for register in &declarations {
        validate_values(register)?;
    }
    let mut expanded: Vec<(usize, usize)> = Vec::new();
    for register in &decl.fixed {
        add_expanded(&mut expanded, register, register.offset.0, decl.size.0)?;
    }
    for bank in &decl.banks {
        if bank.stride.0 == 0 {
            return Err(syn::Error::new(
                bank.stride.1,
                "bank stride must be nonzero",
            ));
        }
        let alignment = bank
            .registers
            .iter()
            .map(|register| register.width.0)
            .max()
            .unwrap_or(1);
        if bank.stride.0 % alignment != 0 {
            return Err(syn::Error::new(bank.stride.1, "bank stride is misaligned"));
        }
        let extent = bank
            .registers
            .iter()
            .try_fold(0_usize, |largest, register| {
                register
                    .offset
                    .0
                    .checked_add(register.width.0)
                    .map(|end| largest.max(end))
            })
            .ok_or_else(|| syn::Error::new(bank.stride.1, "bank member extent overflows usize"))?;
        if bank.stride.0 < extent {
            return Err(syn::Error::new(
                bank.stride.1,
                "bank stride is smaller than member extent",
            ));
        }
        for instance in 0..bank.count.0 {
            for register in &bank.registers {
                let offset = instance
                    .checked_mul(bank.stride.0)
                    .and_then(|part| bank.base.0.checked_add(part))
                    .and_then(|base| base.checked_add(register.offset.0))
                    .ok_or_else(|| {
                        syn::Error::new(bank.base.1, "expanded bank offset overflows usize")
                    })?;
                add_expanded(&mut expanded, register, offset, decl.size.0)?;
            }
        }
    }
    Ok(())
}

fn add_expanded(
    ranges: &mut Vec<(usize, usize)>,
    register: &RegisterDecl,
    offset: usize,
    size: usize,
) -> syn::Result<()> {
    let end = offset
        .checked_add(register.width.0)
        .ok_or_else(|| syn::Error::new(register.offset.1, "register extent overflows usize"))?;
    if end > size {
        return Err(syn::Error::new(
            register.offset.1,
            "register extends beyond map size",
        ));
    }
    if ranges
        .iter()
        .any(|&(start, prior_end)| start < end && offset < prior_end)
    {
        return Err(syn::Error::new(
            register.offset.1,
            "expanded register declarations collide",
        ));
    }
    ranges.push((offset, end));
    Ok(())
}
