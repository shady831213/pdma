use std::collections::HashSet;

use crate::internal::syntax::{
    BitsDecl, EnumDecl, ModernRecordDecl, ModernTypeDecl, ScalarType, UnionDecl,
};

pub(in crate::internal) fn check(decl: &ModernRecordDecl) -> syn::Result<()> {
    super::validate_endian(decl.endian.to_string().as_str(), decl.endian.span())?;
    if let Some((alignment, span)) = decl.alignment {
        if alignment == 0 || !alignment.is_power_of_two() {
            return Err(syn::Error::new(
                span,
                "record alignment must be a nonzero power of two",
            ));
        }
    }
    let mut names = HashSet::new();
    for field in &decl.fields {
        if !names.insert(field.name.to_string()) {
            return Err(syn::Error::new(field.name.span(), "duplicate field name"));
        }
        check_type(decl, &field.ty)?;
    }
    Ok(())
}

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
            None => previous.map_or(0, |value: u64| value.saturating_add(1)),
        };
        if value > decl.storage.maximum() {
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

fn check_type(decl: &ModernRecordDecl, ty: &ModernTypeDecl) -> syn::Result<()> {
    match ty {
        ModernTypeDecl::Scalar { constant, .. } => {
            if let Some(path) = constant {
                let terminal = path.segments.last().map(|segment| &segment.ident);
                if terminal.is_none_or(|terminal| terminal != "record_size") {
                    return Err(syn::Error::new_spanned(
                        path,
                        "constant field must reference `record_size`",
                    ));
                }
            }
            Ok(())
        }
        ModernTypeDecl::Enum(enum_decl) => enum_variants(enum_decl).map(|_| ()),
        ModernTypeDecl::Bits(bits) => check_bits(bits),
        ModernTypeDecl::Record(path) => {
            if path
                .segments
                .last()
                .is_some_and(|segment| segment.ident == decl.name)
            {
                Err(syn::Error::new_spanned(
                    path,
                    "record definition recursively references itself",
                ))
            } else {
                Ok(())
            }
        }
        ModernTypeDecl::Union(union) => check_union(decl, union),
    }
}

fn check_bits(bits: &BitsDecl) -> syn::Result<()> {
    let storage_bits = bits.storage.width() * 8;
    let mut ranges = Vec::new();
    let mut names = HashSet::new();
    for field in &bits.fields {
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

fn check_union(decl: &ModernRecordDecl, union: &UnionDecl) -> syn::Result<()> {
    if union.cases.is_empty() {
        return Err(syn::Error::new(union.tag.span, "union requires a case"));
    }
    if union.tag.segments.len() > 2 {
        return Err(syn::Error::new(
            union.tag.span,
            "tag path may select a local enum or one nested-record enum",
        ));
    }
    let root = union
        .tag
        .segments
        .first()
        .ok_or_else(|| syn::Error::new(union.tag.span, "union tag path must select a field"))?;
    let root_field = decl
        .fields
        .iter()
        .find(|field| field.name == *root)
        .ok_or_else(|| syn::Error::new(root.span(), "union tag root does not exist"))?;
    let mut names = HashSet::new();
    let mut storage = None;
    let mut field_count = 0_usize;
    for case in &union.cases {
        if !names.insert(case.name.to_string()) {
            return Err(syn::Error::new(case.name.span(), "duplicate union case"));
        }
        let mut field_names = HashSet::new();
        for field in &case.fields {
            field_count += 1;
            if !field_names.insert(field.name.to_string()) {
                return Err(syn::Error::new(
                    field.name.span(),
                    "duplicate union field name",
                ));
            }
            if field.storage.width() < field.scalar.width() {
                return Err(syn::Error::new(
                    field.name.span(),
                    "union field storage is narrower than its value",
                ));
            }
            if storage.is_some_and(|storage: ScalarType| storage.width() != field.storage.width()) {
                return Err(syn::Error::new(
                    field.name.span(),
                    "union fields must use one storage width",
                ));
            }
            storage = Some(field.storage);
        }
    }
    if field_count == 0 {
        return Err(syn::Error::new(
            union.tag.span,
            "union requires at least one stored field",
        ));
    }
    match (union.tag.segments.len(), &root_field.ty) {
        (1, ModernTypeDecl::Enum(tag)) => {
            let variants = enum_variants(tag)?;
            let variant_names = variants
                .iter()
                .map(|(name, _)| name.to_string())
                .collect::<HashSet<_>>();
            if names != variant_names {
                return Err(syn::Error::new(
                    union.tag.span,
                    "union cases must exactly match tag enum variants",
                ));
            }
        }
        (1, _) => {
            return Err(syn::Error::new(root.span(), "tag field must be an enum"));
        }
        (2, ModernTypeDecl::Record(_)) => {}
        (2, _) => {
            return Err(syn::Error::new(root.span(), "tag root must be a record"));
        }
        _ => {
            return Err(syn::Error::new(
                union.tag.span,
                "tag path must select one enum field",
            ));
        }
    }
    Ok(())
}
