use std::collections::HashSet;

use proc_macro2::Span;

use crate::syntax::{FieldShape, RecordDecl};

pub(crate) fn check(decl: &RecordDecl) -> syn::Result<()> {
    if decl.alignment.0 == 0 || !decl.alignment.0.is_power_of_two() {
        return Err(syn::Error::new(
            decl.alignment.1,
            "record alignment must be a nonzero power of two",
        ));
    }
    let mut ids = HashSet::new();
    for field in &decl.fields {
        match &field.shape {
            FieldShape::Scalar { id, .. } => insert_id(&mut ids, *id)?,
            FieldShape::Array { ids: field_ids, .. } => {
                for id in field_ids.iter().copied() {
                    insert_id(&mut ids, id)?;
                }
            }
        }
    }
    let mut names = HashSet::new();
    for field in &decl.fields {
        if !names.insert(field.name.to_string()) {
            return Err(syn::Error::new(field.name.span(), "duplicate field name"));
        }
    }
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for field in &decl.fields {
        validate_endian(field.endian.to_string().as_str(), field.endian.span())?;
        match &field.shape {
            FieldShape::Scalar { scalar, .. } => {
                insert_range(
                    &mut ranges,
                    field.offset.0,
                    scalar.width(),
                    field.offset.1,
                    decl.size.0,
                )?;
            }
            FieldShape::Array {
                scalar,
                count,
                ids: field_ids,
                stride,
            } => {
                if count.0 == 0 {
                    return Err(syn::Error::new(count.1, "array count must be nonzero"));
                }
                if field_ids.len() != count.0 {
                    return Err(syn::Error::new(
                        count.1,
                        "array ID count must equal array count",
                    ));
                }
                if stride.0 < scalar.width() {
                    return Err(syn::Error::new(
                        stride.1,
                        "array stride is smaller than scalar width",
                    ));
                }
                if stride.0 % scalar.width() != 0 {
                    return Err(syn::Error::new(
                        stride.1,
                        "array stride is misaligned for scalar width",
                    ));
                }
                for index in 0..field_ids.len() {
                    let offset = index
                        .checked_mul(stride.0)
                        .and_then(|part| field.offset.0.checked_add(part))
                        .ok_or_else(|| {
                            syn::Error::new(field.offset.1, "record array extent overflows usize")
                        })?;
                    insert_range(
                        &mut ranges,
                        offset,
                        scalar.width(),
                        field.offset.1,
                        decl.size.0,
                    )?;
                }
            }
        }
    }
    Ok(())
}

fn validate_endian(value: &str, span: Span) -> syn::Result<()> {
    if matches!(value, "Little" | "Big") {
        Ok(())
    } else {
        Err(syn::Error::new(span, "endian must be Little or Big"))
    }
}

fn insert_id(ids: &mut HashSet<u16>, id: (u16, Span)) -> syn::Result<()> {
    if ids.insert(id.0) {
        Ok(())
    } else {
        Err(syn::Error::new(id.1, "duplicate field ID"))
    }
}

fn insert_range(
    ranges: &mut Vec<(usize, usize)>,
    offset: usize,
    width: usize,
    span: Span,
    size: usize,
) -> syn::Result<()> {
    let end = offset
        .checked_add(width)
        .ok_or_else(|| syn::Error::new(span, "record field extent overflows usize"))?;
    if end > size {
        return Err(syn::Error::new(
            span,
            "record field extends beyond declared size",
        ));
    }
    if ranges
        .iter()
        .any(|&(start, prior_end)| start < end && offset < prior_end)
    {
        return Err(syn::Error::new(span, "record fields overlap"));
    }
    ranges.push((offset, end));
    Ok(())
}
