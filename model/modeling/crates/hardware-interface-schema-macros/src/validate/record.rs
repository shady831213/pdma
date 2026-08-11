use std::collections::HashSet;

use proc_macro2::Span;

use syn::spanned::Spanned;

use crate::syntax::{FieldShape, RecordDecl};

mod names;

pub(crate) fn check(decl: &RecordDecl) -> syn::Result<()> {
    if decl.alignment.0 == 0 || !decl.alignment.0.is_power_of_two() {
        return Err(syn::Error::new(
            decl.alignment.1,
            "record alignment must be a nonzero power of two",
        ));
    }
    validate_ids_and_names(decl)?;
    names::check(decl)?;
    let ranges = validate_fields(decl)?;
    validate_records(decl, &ranges)
}

fn validate_ids_and_names(decl: &RecordDecl) -> syn::Result<()> {
    let mut ids = HashSet::new();
    for field in &decl.fields {
        match &field.shape {
            FieldShape::Scalar { id, .. } | FieldShape::Reference { id, .. } => {
                insert_id(&mut ids, *id)?;
            }
            FieldShape::Array { ids: field_ids, .. } => {
                for id in field_ids.iter().copied() {
                    insert_id(&mut ids, id)?;
                }
            }
        }
    }
    for record in &decl.records {
        insert_id(&mut ids, record.id)?;
    }
    let mut names = HashSet::new();
    for field in &decl.fields {
        if !names.insert(field.name.to_string()) {
            return Err(syn::Error::new(field.name.span(), "duplicate field name"));
        }
    }
    for record in &decl.records {
        if !names.insert(record.name.to_string()) {
            return Err(syn::Error::new(
                record.name.span(),
                "duplicate placement name",
            ));
        }
    }
    Ok(())
}

fn validate_fields(decl: &RecordDecl) -> syn::Result<Vec<(usize, usize)>> {
    let mut ranges = Vec::new();
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
            FieldShape::Reference { .. } => ranges.push((field.offset.0, field.offset.0)),
        }
    }
    Ok(ranges)
}

fn validate_records(decl: &RecordDecl, ranges: &[(usize, usize)]) -> syn::Result<()> {
    for record in &decl.records {
        if record
            .definition
            .segments
            .last()
            .is_some_and(|segment| segment.ident == decl.name)
        {
            return Err(syn::Error::new(
                record.definition.span(),
                "record definition recursively references itself",
            ));
        }
        if ranges.iter().any(|&(start, _)| start == record.offset.0) {
            return Err(syn::Error::new(
                record.offset.1,
                "nested record placement collides with a scalar placement",
            ));
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
