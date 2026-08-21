use std::collections::HashSet;

use crate::internal::syntax::{RegisterDecl, RegisterDefinitionDecl};

pub(in crate::internal) fn check(registers: &[RegisterDecl], overlap: bool) -> syn::Result<()> {
    let mut ids = HashSet::new();
    for register in registers {
        if !ids.insert(register.id.0) {
            return Err(syn::Error::new(register.id.1, "duplicate register ID"));
        }
    }
    let mut names = HashSet::new();
    let mut generated = HashSet::new();
    for register in registers {
        if !names.insert(register.name.value()) {
            return Err(syn::Error::new(
                register.name.span(),
                "duplicate register name",
            ));
        }
        if !generated.insert(generated_ident(&register.symbol.to_string())) {
            return Err(syn::Error::new(
                register.symbol.span(),
                "generated register identifier collision",
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

pub(in crate::internal) fn check_definition(register: &RegisterDefinitionDecl) -> syn::Result<()> {
    if !matches!(register.width.0, 1 | 2 | 4 | 8) {
        return Err(syn::Error::new(
            register.width.1,
            "register width must be 1, 2, 4, or 8",
        ));
    }
    if let Some((alignment, span)) = register.alignment
        && (alignment == 0 || !alignment.is_power_of_two())
    {
        return Err(syn::Error::new(
            span,
            "register alignment must be a nonzero power of two",
        ));
    }
    if !matches!(register.endian.to_string().as_str(), "Little" | "Big") {
        return Err(syn::Error::new(
            register.endian.span(),
            "endian must be Little or Big",
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
    let mut ids = HashSet::new();
    let mut names = HashSet::new();
    let mut generated = HashSet::new();
    for field in &register.fields {
        if !ids.insert(field.id.0) {
            return Err(syn::Error::new(field.id.1, "duplicate register field ID"));
        }
        if !names.insert(field.name.to_string()) {
            return Err(syn::Error::new(
                field.name.span(),
                "duplicate register field name",
            ));
        }
        if !generated.insert(generated_ident(&field.name.to_string())) {
            return Err(syn::Error::new(
                field.name.span(),
                "generated register field identifier collision",
            ));
        }
    }
    Ok(())
}

fn generated_ident(name: &str) -> String {
    name.split('_')
        .map(|part| {
            let mut chars = part.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(chars).collect()
            })
        })
        .collect()
}

pub(in crate::internal) fn validate_values(register: &RegisterDecl) -> syn::Result<()> {
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
