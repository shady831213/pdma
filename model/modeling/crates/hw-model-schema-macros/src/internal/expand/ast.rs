use proc_macro2::TokenStream;
use quote::quote;

use crate::internal::syntax::UntaggedUnionDecl;
mod map;
mod record;
pub(super) use map::expand as register_map;
pub(super) use record::expand as record;

pub(super) fn register(
    decl: &crate::internal::syntax::RegisterDefinitionDecl,
    facade: &TokenStream,
) -> TokenStream {
    let marker = &decl.name;
    let source = match &decl.fields {
        crate::internal::syntax::RegisterFieldsDecl::Inline(_) => TokenStream::new(),
        crate::internal::syntax::RegisterFieldsDecl::Reference(path) => {
            quote!(.with_register_source(
                #facade::ast::Reference::new("fields", #facade::ast::ReferenceKind::Field,
                    <#path as #facade::BitsLayout>::schema_definition)
            ))
        }
    };
    quote! {
        #facade::__register_schema!(<#marker as #facade::ast::DescribeSchema>::schema_definition);
        impl #facade::ast::DescribeSchema for #marker {
            fn schema_definition() -> #facade::ast::Definition {
                #facade::ast::Definition::register::<Self, _>(Self::SPEC)#source
            }
        }
    }
}

pub(super) fn union(decl: &UntaggedUnionDecl, facade: &TokenStream) -> TokenStream {
    let marker = &decl.name;
    let references = decl.cases.iter().map(|(name, path)| {
        let name = name.to_string();
        quote! {
            #facade::ast::Reference::new(#name, #facade::ast::ReferenceKind::Variant,
                <#path as #facade::SchemaLayout>::schema_definition)
        }
    });
    quote! {
        #facade::__register_schema!(<#marker as #facade::ast::DescribeSchema>::schema_definition);
        impl #facade::ast::DescribeSchema for #marker {
            fn schema_definition() -> #facade::ast::Definition {
                #facade::ast::Definition::union::<Self>(Self::SPEC, ::core::option::Option::None, ::std::vec![#(#references),*])
            }
        }
    }
}
