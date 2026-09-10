use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::internal::{crate_path, syntax::BitsSchemaDecl, validate};

mod codec;
mod marker;
mod values;

struct SchemaContext<'a> {
    decl: &'a BitsSchemaDecl,
    facade: TokenStream,
    values: syn::Ident,
    field_id: syn::Ident,
    bit_width: usize,
    extent: usize,
    declared_mask: u64,
}

pub(super) fn expand(decl: &BitsSchemaDecl) -> syn::Result<TokenStream> {
    validate::bits::check_schema(&decl.fields, decl.storage.width() * 8)?;
    let context = SchemaContext {
        decl,
        facade: crate_path::facade()?,
        values: format_ident!("{}Values", decl.name),
        field_id: format_ident!("{}FieldId", decl.name),
        bit_width: validate::bits::bit_width(&decl.fields),
        extent: decl.storage.width(),
        declared_mask: validate::bits::declared_mask(&decl.fields),
    };
    let identities = identities(&context);
    let enums = super::enum_items(&decl.name, &decl.fields, &decl.visibility, &context.facade)?;
    let values = values::expand(&context);
    let marker = marker::expand(&context)?;
    Ok(quote!(#identities #enums #values #marker))
}

fn identities(context: &SchemaContext<'_>) -> TokenStream {
    let visibility = &context.decl.visibility;
    let field_id = &context.field_id;
    let variants = context.decl.fields.iter().map(|field| {
        let variant = super::super::register::variant(&field.name.to_string());
        quote!(#[doc = "Stable generated field ID."] #variant)
    });
    quote! {
        #[doc = "Stable identities for one reusable bits schema."]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        #visibility enum #field_id { #(#variants),* }
    }
}
