use std::collections::HashSet;

use crate::internal::syntax::BitFieldDecl;

pub(in crate::internal) fn check(fields: &[BitFieldDecl], storage_bits: usize) -> syn::Result<()> {
    let mut names = HashSet::new();
    let mut ranges = Vec::with_capacity(fields.len());
    for field in fields {
        if !names.insert(field.name.to_string()) {
            return Err(syn::Error::new(
                field.name.span(),
                "duplicate bits field name",
            ));
        }
        if field.start.0 >= field.end.0 || field.end.0 > storage_bits {
            return Err(syn::Error::new(
                field.start.1,
                "bits field range is empty or exceeds storage",
            ));
        }
        if ranges
            .iter()
            .any(|&(start, end)| start < field.end.0 && field.start.0 < end)
        {
            return Err(syn::Error::new(field.start.1, "bits fields overlap"));
        }
        ranges.push((field.start.0, field.end.0));
    }
    Ok(())
}

pub(in crate::internal) fn check_schema(fields: &[BitFieldDecl]) -> syn::Result<()> {
    if fields.is_empty() {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "bits schema requires at least one field",
        ));
    }
    if fields.len() > usize::from(u16::MAX) {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "bits schema has too many fields",
        ));
    }
    check(fields, 64)
}

pub(in crate::internal) fn bit_width(fields: &[BitFieldDecl]) -> usize {
    fields.iter().map(|field| field.end.0).max().unwrap_or(0)
}

pub(in crate::internal) fn extent(fields: &[BitFieldDecl]) -> usize {
    match bit_width(fields) {
        0..=8 => 1,
        9..=16 => 2,
        17..=32 => 4,
        _ => 8,
    }
}

pub(in crate::internal) fn declared_mask(fields: &[BitFieldDecl]) -> u64 {
    fields.iter().fold(0, |mask, field| {
        mask | (bit_mask(field.end.0 - field.start.0) << field.start.0)
    })
}

pub(in crate::internal) const fn bit_mask(width: usize) -> u64 {
    if width >= 64 {
        u64::MAX
    } else {
        (1_u64 << width) - 1
    }
}
