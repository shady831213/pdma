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
    let selector = format!("self.{selector}");
    let mut cases = Vec::new();
    let mut descriptions = Vec::new();
    for case in &decl.cases {
        let case_type = super::case_type(record, field, &case.name);
        let variant = &case.name;
        let label = variant.to_string();
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
        let logical_fields = case.fields.iter().enumerate().map(|(index, field)| {
            let name = field.name.to_string();
            let ty = match &field.ty {
                TaggedUnionFieldType::Scalar(scalar) => {
                    let kind = crate::internal::expand::scalar::kind(*scalar);
                    quote!(#facade::ast::FieldType::Scalar {
                        definition: #facade::FieldDefinition::new(#facade::ScalarKind::#kind).with_endian(#facade::Endian::#endian),
                        constant: ::core::option::Option::None,
                    })
                }
                TaggedUnionFieldType::Bits(path) => quote!(#facade::ast::FieldType::Reference(
                    #facade::ast::Reference::new(#name, #facade::ast::ReferenceKind::Field, <#path as #facade::BitsLayout>::schema_definition))),
            };
            quote!(#facade::ast::Field::new(#name, #index * #quantum, #quantum, #facade::Endian::#endian, #ty))
        }).collect::<Vec<_>>();
        cases.push(quote!({
            const SPEC: #facade::RecordSpec = #facade::RecordSpec::named(#label, #extent, #quantum, &[#(#fields),*], &[]);
            #facade::ast::Case::inline(#label, ::core::option::Option::Some(::core::convert::Into::<u64>::into(#tag::#variant.raw())),
                #facade::ast::RecordDefinition::new(SPEC, ::std::vec![#(#logical_fields),*]).with_declaration(::core::option::Option::None, #facade::Endian::#endian))
        }));
        descriptions.push(quote! {
            impl #facade::ast::DescribeSchema for #case_type {
                fn schema_definition() -> #facade::ast::Definition {
                    const SPEC: #facade::RecordSpec = #facade::RecordSpec::named(stringify!(#case_type), #extent, #quantum, &[#(#fields),*], &[]);
                    #facade::ast::Definition::record::<Self>(SPEC, ::std::vec![#(#logical_fields),*]).with_record_declaration(::core::option::Option::None, #facade::Endian::#endian)
                }
            }
        });
    }
    Ok(quote! {
        #(#descriptions)*
        impl #facade::ast::DescribeUnion for #name {
            fn schema_union() -> #facade::ast::UnionDefinition {
                const SPEC: #facade::RecordSpec = #facade::RecordSpec::named(stringify!(#name), #extent, #quantum, &[], &[]);
                #facade::ast::UnionDefinition::new(SPEC, ::core::option::Option::Some(#selector), ::std::vec![#(#cases),*])
            }
        }
        impl #facade::ast::DescribeSchema for #name {
            fn schema_definition() -> #facade::ast::Definition {
                #facade::ast::Definition::inline_union::<Self>(<Self as #facade::ast::DescribeUnion>::schema_union())
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
    let selector = format!("self.{selector}");
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
                #facade::ast::Case::external(#facade::ast::Reference::new(#label, #facade::ast::ReferenceKind::Variant,
                    <#path as #facade::SchemaLayout>::schema_definition)
                    .with_discriminant(::core::convert::Into::<u64>::into(#tag::#variant.raw())))
            })
        })
        .collect::<syn::Result<Vec<_>>>()?;
    Ok(quote! {
        impl #facade::ast::DescribeUnion for #name {
            fn schema_union() -> #facade::ast::UnionDefinition {
                #facade::ast::UnionDefinition::new(Self::SPEC, ::core::option::Option::Some(#selector), ::std::vec![#(#references),*])
            }
        }
        impl #facade::ast::DescribeSchema for #name {
            fn schema_definition() -> #facade::ast::Definition {
                #facade::ast::Definition::inline_union::<Self>(<Self as #facade::ast::DescribeUnion>::schema_union())
            }
        }
    })
}
