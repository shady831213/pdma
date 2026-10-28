use proc_macro2::TokenStream;
use syn::ext::IdentExt;

use super::layout::{RecordPlan, tagged_union_geometry};

use crate::internal::syntax::{ModernRecordDecl, ModernTypeDecl};
use crate::internal::validate::members::Members;

pub(super) fn check(
    decl: &ModernRecordDecl,
    plan: &RecordPlan<'_>,
    facade: &TokenStream,
) -> syn::Result<()> {
    let mut owned = Members::new(&[
        "from_bytes",
        "to_bytes",
        "encode_into",
        "encoded",
        "ZERO",
        "DOCUMENTATION",
        "FIELD_COUNT",
        "RECORD_COUNT",
        "RECORD_ALIGNMENT",
        "RECORD_SIZE",
        "FIELDS",
        "RECORDS",
        "SPEC",
    ]);
    let mut views = Members::new(&["new", "values", "set_values"]);
    for field in &plan.fields {
        let name = &field.decl.name;
        owned.insert(name, &field.offset_name.to_string())?;
        owned.insert(
            name,
            &format!("{}_IDENTITY", name.unraw().to_string().to_ascii_uppercase()),
        )?;
        if let ModernTypeDecl::TaggedUnion(union) = &field.decl.ty
            && let Some(geometry) = tagged_union_geometry(union, facade)
        {
            for index in 0..geometry.slots {
                let word = if field.scoped_tagged_union {
                    format!("{}_word{index}", name.unraw())
                } else {
                    format!("word{index}")
                };
                owned.insert(name, &format!("{}_OFFSET", word.to_ascii_uppercase()))?;
                views.insert(name, &word)?;
                views.insert(name, &format!("set_{word}"))?;
            }
        }
    }
    let stored = decl
        .fields
        .iter()
        .filter(|field| {
            !matches!(
                field.ty,
                ModernTypeDecl::Scalar {
                    constant: Some(_),
                    ..
                }
            )
        })
        .count();
    if stored <= 6 {
        owned.insert(&decl.name, "new")?;
    }
    for field in &decl.fields {
        let name = &field.name;
        if !matches!(
            field.ty,
            ModernTypeDecl::Scalar {
                constant: Some(_),
                ..
            }
        ) {
            owned.insert(name, &name.to_string())?;
        }
        match &field.ty {
            ModernTypeDecl::TaggedUnion(_) => {}
            ModernTypeDecl::Codec(_) => {
                views.insert(name, &name.to_string())?;
                views.insert(name, &format!("{}_mut", name.unraw()))?;
            }
            ModernTypeDecl::Scalar { .. }
            | ModernTypeDecl::Enum(_)
            | ModernTypeDecl::Bits(_)
            | ModernTypeDecl::BitsReference(_) => {
                views.insert(name, &name.to_string())?;
                views.insert(name, &format!("set_{}", name.unraw()))?;
            }
        }
    }
    Ok(())
}
