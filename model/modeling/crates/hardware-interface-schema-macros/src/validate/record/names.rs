use std::collections::HashSet;

use proc_macro2::Span;

use crate::internal::syntax::{FieldShape, RecordDecl};

pub(super) fn check(decl: &RecordDecl) -> syn::Result<()> {
    let composable = !decl.records.is_empty()
        || decl
            .fields
            .iter()
            .any(|field| matches!(field.shape, FieldShape::Reference { .. }));
    let mut variants = HashSet::new();
    let mut constants = HashSet::new();
    let mut immutable_methods = HashSet::from([String::from("new")]);
    let mut mutable_methods = HashSet::from([String::from("new")]);
    for field in &decl.fields {
        let name = field.name.to_string();
        let upper = name.to_uppercase();
        insert(&mut constants, format!("{upper}_OFFSET"), field.name.span())?;
        if composable {
            insert(
                &mut constants,
                format!("{upper}_DEFINITION"),
                field.name.span(),
            )?;
        }
        insert(&mut immutable_methods, name.clone(), field.name.span())?;
        insert(&mut mutable_methods, name.clone(), field.name.span())?;
        insert(
            &mut mutable_methods,
            format!("set_{name}"),
            field.name.span(),
        )?;
        let count = match &field.shape {
            FieldShape::Array { ids, .. } => ids.len(),
            FieldShape::Scalar { .. } | FieldShape::Reference { .. } => 1,
        };
        for index in 0..count {
            let source = if count == 1 {
                name.clone()
            } else {
                format!("{name}{index}")
            };
            insert(&mut variants, variant(&source), field.name.span())?;
        }
    }
    for record in &decl.records {
        let name = record.name.to_string();
        let upper = name.to_uppercase();
        insert(
            &mut constants,
            format!("{upper}_OFFSET"),
            record.name.span(),
        )?;
        insert(
            &mut constants,
            format!("{upper}_DEFINITION"),
            record.name.span(),
        )?;
        insert(&mut immutable_methods, name.clone(), record.name.span())?;
        insert(&mut mutable_methods, name.clone(), record.name.span())?;
        insert(
            &mut mutable_methods,
            format!("{name}_mut"),
            record.name.span(),
        )?;
        insert(&mut variants, variant(&name), record.name.span())?;
    }
    Ok(())
}

fn insert(names: &mut HashSet<String>, name: String, span: Span) -> syn::Result<()> {
    if names.insert(name) {
        Ok(())
    } else {
        Err(syn::Error::new(
            span,
            "generated placement identifier collision",
        ))
    }
}

fn variant(name: &str) -> String {
    name.split('_')
        .map(|part| {
            let mut chars = part.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(chars).collect()
            })
        })
        .collect()
}
