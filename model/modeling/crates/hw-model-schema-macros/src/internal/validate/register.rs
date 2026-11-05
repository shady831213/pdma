use std::collections::HashSet;
use syn::ext::IdentExt;

use crate::internal::syntax::{RegisterDefinitionDecl, RegisterFieldsDecl};

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
    let RegisterFieldsDecl::Inline(fields) = &register.fields else {
        return Ok(());
    };
    crate::internal::validate::bits::check(fields, register.width.0 * 8)?;
    let mut generated = HashSet::new();
    let mut views = super::members::Members::new(&["new", "value", "set_value"]);
    let mut owned = super::members::Members::new(&["new", "raw", "ZERO", "RESET"]);
    for field in fields {
        let name = field.name.unraw().to_string();
        views.insert(&field.name, &name)?;
        views.insert(&field.name, &format!("set_{name}"))?;
        owned.insert(&field.name, &name)?;
        owned.insert(&field.name, &format!("with_{name}"))?;
        if !generated.insert(generated_ident(&name)) {
            return Err(syn::Error::new(
                field.name.span(),
                "generated register field identifier collision",
            ));
        }
    }
    if fields.len() > usize::from(u16::MAX) {
        return Err(syn::Error::new(
            register.name.span(),
            "register has too many bit fields",
        ));
    }
    let declared_mask = crate::internal::validate::bits::declared_mask(fields);
    if !fields.is_empty() && register.mask.0 & !declared_mask != 0 {
        return Err(syn::Error::new(
            register.mask.1,
            "writable mask contains undeclared bits",
        ));
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

const fn fits(value: u64, width: usize) -> bool {
    width == 8 || value < (1_u64 << (width * 8))
}
