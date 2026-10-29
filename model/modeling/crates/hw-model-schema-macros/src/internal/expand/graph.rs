use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::internal::syntax::{ModernRecordDecl, ModernTypeDecl, UntaggedUnionDecl};
mod map;
pub(super) use map::expand as register_map;

pub(super) fn register(
    decl: &crate::internal::syntax::RegisterDefinitionDecl,
    facade: &TokenStream,
) -> TokenStream {
    let marker = &decl.name;
    let references = match &decl.fields {
        crate::internal::syntax::RegisterFieldsDecl::Inline(_) => quote!(::std::vec::Vec::new()),
        crate::internal::syntax::RegisterFieldsDecl::Reference(path) => quote!(::std::vec![
            #facade::graph::Reference::new("fields", #facade::graph::ReferenceKind::Field,
                <#path as #facade::BitsLayout>::schema_definition)
        ]),
    };
    quote! {
        #facade::__register_schema!(<#marker as #facade::graph::DescribeSchema>::schema_definition);
        impl #facade::graph::DescribeSchema for #marker {
            fn schema_definition() -> #facade::graph::Definition {
                #facade::graph::Definition::register::<Self, _>(Self::SPEC).with_references(#references)
            }
        }
    }
}

pub(super) fn record(decl: &ModernRecordDecl, facade: &TokenStream) -> TokenStream {
    let marker = &decl.name;
    let references = decl.fields.iter().filter_map(|field| {
        let name = field.name.to_string();
        match &field.ty {
            ModernTypeDecl::Codec(path) => Some(quote! {
                #facade::graph::Reference::new(#name, #facade::graph::ReferenceKind::Field,
                    <#path as #facade::SchemaLayout>::schema_definition)
            }),
            ModernTypeDecl::BitsReference(path) => Some(quote! {
                #facade::graph::Reference::new(#name, #facade::graph::ReferenceKind::Field,
                    <#path as #facade::BitsLayout>::schema_definition)
            }),
            ModernTypeDecl::TaggedUnion(_) => {
                let field_type =
                    format_ident!("{}{}Values", marker, super::register::variant(&name));
                Some(quote! {
                    #facade::graph::Reference::new(#name, #facade::graph::ReferenceKind::Field,
                        <#field_type as #facade::graph::DescribeSchema>::schema_definition)
                })
            }
            ModernTypeDecl::Scalar { .. } | ModernTypeDecl::Enum(_) | ModernTypeDecl::Bits(_) => {
                None
            }
        }
    });
    quote! {
        #facade::__register_schema!(<#marker as #facade::graph::DescribeSchema>::schema_definition);
        impl #facade::graph::DescribeSchema for #marker {
            fn schema_definition() -> #facade::graph::Definition {
                #facade::graph::Definition::record::<Self>(Self::SPEC, ::std::vec![#(#references),*])
            }
        }
    }
}

pub(super) fn union(decl: &UntaggedUnionDecl, facade: &TokenStream) -> TokenStream {
    let marker = &decl.name;
    let references = decl.cases.iter().map(|(name, path)| {
        let name = name.to_string();
        quote! {
            #facade::graph::Reference::new(#name, #facade::graph::ReferenceKind::Variant,
                <#path as #facade::SchemaLayout>::schema_definition)
        }
    });
    quote! {
        #facade::__register_schema!(<#marker as #facade::graph::DescribeSchema>::schema_definition);
        impl #facade::graph::DescribeSchema for #marker {
            fn schema_definition() -> #facade::graph::Definition {
                #facade::graph::Definition::union::<Self>(Self::SPEC, ::core::option::Option::None, ::std::vec![#(#references),*])
            }
        }
    }
}
