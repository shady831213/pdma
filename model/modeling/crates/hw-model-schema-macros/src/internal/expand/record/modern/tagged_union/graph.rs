use crate::internal::syntax::{ModernRecordDecl, TaggedUnionDecl, TaggedUnionFieldType};
use proc_macro2::TokenStream;
use quote::quote;

pub(super) struct Context<'a> {
    pub record: &'a ModernRecordDecl,
    pub field: &'a syn::Ident,
    pub decl: &'a TaggedUnionDecl,
    pub facade: &'a TokenStream,
    pub tag: &'a TokenStream,
    pub extent: &'a TokenStream,
    pub quantum: &'a TokenStream,
    pub endian: &'a syn::Ident,
}

pub(super) fn expand(context: &Context<'_>) -> syn::Result<TokenStream> {
    let Context {
        record,
        field,
        decl,
        facade,
        tag,
        extent,
        quantum,
        endian,
    } = context;
    let name = super::scalar::type_name(&record.name, field, "Values");
    let selector = decl
        .tag
        .segments
        .iter()
        .map(syn::Ident::to_string)
        .collect::<Vec<_>>()
        .join(".");
    let mut cases = Vec::new();
    let mut descriptions = Vec::new();
    for case in &decl.cases {
        let case_type = super::case_type(record, field, &case.name);
        let variant = &case.name;
        let label = variant.to_string();
        cases.push(
            quote!(#facade::graph::Reference::new(#label, #facade::graph::ReferenceKind::Variant,
            <#case_type as #facade::graph::DescribeSchema>::schema_definition)
            .with_discriminant(::core::convert::Into::<u64>::into(#tag::#variant.raw()))),
        );
        let fields = case.fields.iter().enumerate().map(|(index, field)| {
            let name = field.name.to_string();
            let id = u16::try_from(index + 1).map_err(|_| syn::Error::new(field.name.span(), "too many union case fields"))?;
            let definition = match &field.ty {
                TaggedUnionFieldType::Scalar(scalar) => {
                    let kind = crate::internal::expand::scalar::kind(*scalar);
                    quote!(#facade::FieldDefinition::new(#facade::ScalarKind::#kind))
                }
                TaggedUnionFieldType::Bits(path) => quote!(#facade::FieldDefinition::new(<#path as #facade::BitsLayout>::SCALAR_KIND).with_bits(<#path as #facade::BitsLayout>::FIELDS)),
            };
            Ok(quote!(#facade::FieldSpec::place(#id, #name, #index * #quantum, (#definition).with_endian(#facade::Endian::#endian))))
        }).collect::<syn::Result<Vec<_>>>()?;
        let references = case.fields.iter().filter_map(|field| {
            let name = field.name.to_string();
            match &field.ty {
                TaggedUnionFieldType::Scalar(_) => None,
                TaggedUnionFieldType::Bits(path) => Some(quote!(#facade::graph::Reference::new(#name, #facade::graph::ReferenceKind::Field, <#path as #facade::BitsLayout>::schema_definition))),
            }
        });
        descriptions.push(quote! {
            impl #facade::graph::DescribeSchema for #case_type {
                fn schema_definition() -> #facade::graph::Definition {
                    const SPEC: #facade::RecordSpec = #facade::RecordSpec::named(stringify!(#case_type), #extent, #quantum, &[#(#fields),*], &[]);
                    #facade::graph::Definition::record::<Self>(SPEC, ::std::vec![#(#references),*])
                }
            }
        });
    }
    Ok(quote! {
        #(#descriptions)*
        impl #facade::graph::DescribeSchema for #name {
            fn schema_definition() -> #facade::graph::Definition {
                const SPEC: #facade::RecordSpec = #facade::RecordSpec::named(stringify!(#name), #extent, #quantum, &[], &[]);
                #facade::graph::Definition::union::<Self>(SPEC, ::core::option::Option::Some(#selector), ::std::vec![#(#cases),*])
            }
        }
    })
}

pub(super) fn codecs(
    name: &syn::Ident,
    decl: &TaggedUnionDecl,
    tag: &TokenStream,
    facade: &TokenStream,
) -> syn::Result<TokenStream> {
    let selector = decl
        .tag
        .segments
        .iter()
        .map(syn::Ident::to_string)
        .collect::<Vec<_>>()
        .join(".");
    let references = decl
        .cases
        .iter()
        .map(|case| {
            let path = case
                .codec
                .as_ref()
                .ok_or_else(|| syn::Error::new(case.name.span(), "expected codec case"))?;
            let variant = &case.name;
            let label = variant.to_string();
            Ok(quote! {
                #facade::graph::Reference::new(#label, #facade::graph::ReferenceKind::Variant,
                    <#path as #facade::SchemaLayout>::schema_definition)
                    .with_discriminant(::core::convert::Into::<u64>::into(#tag::#variant.raw()))
            })
        })
        .collect::<syn::Result<Vec<_>>>()?;
    Ok(quote! {
        impl #facade::graph::DescribeSchema for #name {
            fn schema_definition() -> #facade::graph::Definition {
                #facade::graph::Definition::union::<Self>(Self::SPEC, ::core::option::Option::Some(#selector), ::std::vec![#(#references),*])
            }
        }
    })
}
