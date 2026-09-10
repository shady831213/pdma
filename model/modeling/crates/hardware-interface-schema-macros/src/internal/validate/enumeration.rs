use std::collections::HashSet;

use crate::internal::syntax::EnumDecl;

pub(in crate::internal) fn enum_variants(decl: &EnumDecl) -> syn::Result<Vec<(syn::Ident, u64)>> {
    if decl.variants.is_empty() {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "enum requires at least one variant",
        ));
    }
    let mut names = HashSet::new();
    let mut values = HashSet::new();
    let mut previous = None;
    let mut output = Vec::with_capacity(decl.variants.len());
    for variant in &decl.variants {
        if !names.insert(variant.name.to_string()) {
            return Err(syn::Error::new(
                variant.name.span(),
                "duplicate enum variant name",
            ));
        }
        let value = match variant.value {
            Some((value, _)) => value,
            None => match previous {
                None => 0,
                Some(value) => u64::checked_add(value, 1).ok_or_else(|| {
                    syn::Error::new(variant.name.span(), "enum variant exceeds storage width")
                })?,
            },
        };
        if value > super::bits::bit_mask(decl.bit_width) {
            return Err(syn::Error::new(
                variant.name.span(),
                "enum variant exceeds storage width",
            ));
        }
        if !values.insert(value) {
            return Err(syn::Error::new(
                variant.name.span(),
                "duplicate enum variant value",
            ));
        }
        previous = Some(value);
        output.push((variant.name.clone(), value));
    }
    Ok(output)
}
