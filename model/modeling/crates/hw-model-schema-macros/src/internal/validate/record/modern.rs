use std::collections::HashSet;

use crate::internal::syntax::{ModernRecordDecl, ModernTypeDecl, TaggedUnionDecl};
use crate::internal::validate::enumeration::enum_variants;

pub(in crate::internal) fn check(decl: &ModernRecordDecl) -> syn::Result<()> {
    super::validate_endian(decl.endian.to_string().as_str(), decl.endian.span())?;
    if let Some((alignment, span)) = decl.alignment
        && (alignment == 0 || !alignment.is_power_of_two())
    {
        return Err(syn::Error::new(
            span,
            "record alignment must be a nonzero power of two",
        ));
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
        ModernTypeDecl::Bits(bits) => {
            crate::internal::validate::bits::check(&bits.fields, bits.storage.width() * 8)
        }
        ModernTypeDecl::BitsReference(_) => Ok(()),
        ModernTypeDecl::Codec(path) => {
            if path
                .get_ident()
                .is_some_and(|name| *name == decl.name || name == "Self")
            {
                Err(syn::Error::new_spanned(
                    path,
                    "record definition recursively references itself",
                ))
            } else {
                Ok(())
            }
        }
        ModernTypeDecl::TaggedUnion(union) => check_tagged_union(decl, union),
    }
}

fn check_tagged_union(decl: &ModernRecordDecl, union: &TaggedUnionDecl) -> syn::Result<()> {
    if union.cases.is_empty() {
        return Err(syn::Error::new(union.tag.span, "union requires a case"));
    }
    if union.tag.segments.len() > 2 {
        return Err(syn::Error::new(
            union.tag.span,
            "tag path may select a local enum or one record/bits member",
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
    let names = check_tagged_union_cases(decl, union)?;
    let enumeration = match (union.tag.segments.as_slice(), &root_field.ty) {
        ([_], ModernTypeDecl::Enum(tag)) => Some(tag),
        ([_, member], ModernTypeDecl::Bits(bits)) => {
            let field = bits
                .fields
                .iter()
                .find(|field| field.name == *member)
                .ok_or_else(|| {
                    syn::Error::new(member.span(), "union tag bitfield does not exist")
                })?;
            Some(field.enumeration.as_ref().ok_or_else(|| {
                syn::Error::new(member.span(), "union tag bitfield must be an enum")
            })?)
        }
        ([_], _) => {
            return Err(syn::Error::new(root.span(), "tag field must be an enum"));
        }
        ([_, _], ModernTypeDecl::Codec(_) | ModernTypeDecl::BitsReference(_)) => None,
        ([_, _], _) => {
            return Err(syn::Error::new(
                root.span(),
                "tag root must be a record or bits field",
            ));
        }
        _ => {
            return Err(syn::Error::new(
                union.tag.span,
                "tag path must select one enum field",
            ));
        }
    };
    if let Some(tag) = enumeration {
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
    Ok(())
}

fn check_tagged_union_cases(
    decl: &ModernRecordDecl,
    union: &TaggedUnionDecl,
) -> syn::Result<HashSet<String>> {
    let mut names = HashSet::new();
    let mut field_count = 0_usize;
    for case in &union.cases {
        if let Some(path) = &case.codec {
            field_count += 1;
            check_type(decl, &ModernTypeDecl::Codec(path.clone()))?;
        }
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
        }
    }
    if field_count == 0 {
        return Err(syn::Error::new(
            union.tag.span,
            "union requires at least one stored field",
        ));
    }
    let codec_cases = union
        .cases
        .iter()
        .filter(|case| case.codec.is_some())
        .count();
    if codec_cases != 0 && codec_cases != union.cases.len() {
        return Err(syn::Error::new(
            union.tag.span,
            "codec and inline union cases cannot be mixed",
        ));
    }
    Ok(names)
}
