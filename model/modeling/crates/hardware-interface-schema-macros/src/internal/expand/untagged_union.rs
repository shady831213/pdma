use crate::internal::{crate_path, syntax::UntaggedUnionDecl};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
mod storage;

pub(in crate::internal) fn run(input: TokenStream) -> TokenStream {
    syn::parse2::<UntaggedUnionDecl>(input)
        .and_then(|decl| expand(&decl))
        .unwrap_or_else(syn::Error::into_compile_error)
}

fn expand(decl: &UntaggedUnionDecl) -> syn::Result<TokenStream> {
    let facade = crate_path::facade()?;
    let name = &decl.name;
    let visibility = &decl.visibility;
    let case_type = format_ident!("{name}Case");
    let docs = decl.documentation.rust_docs();
    let metadata = decl.documentation.metadata(&facade);
    let cases: Vec<_> = decl.cases.iter().map(|(case, _)| case).collect();
    let records: Vec<_> = decl.cases.iter().map(|(_, record)| record).collect();
    let Some((first, record)) = decl.cases.first() else {
        return Err(syn::Error::new(
            name.span(),
            "union requires at least one case",
        ));
    };
    let storage = storage::expand(decl, &facade);
    Ok(quote! {
        #docs
        #[doc = "Semantic value of an explicitly selected complete-record union."]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #visibility enum #name { #(#[doc = "Complete record interpretation."] #cases(#records)),* }
        #[doc = "Caller-selected interpretation of untagged union storage."]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #visibility enum #case_type { #(#[doc = "Selects this complete record interpretation."] #cases),* }
        impl #name {
            #[doc = "Maximum natural alignment of the complete cases."]
            pub const RECORD_ALIGNMENT: usize = {
                let mut alignment = 1;
                #(assert!(<#records as #facade::RecordLayout>::ALIGNMENT.is_power_of_two(), "union case alignment must be a nonzero power of two");)*
                #(if <#records as #facade::RecordLayout>::ALIGNMENT > alignment {
                    alignment = <#records as #facade::RecordLayout>::ALIGNMENT;
                })*
                alignment
            };
            #[doc = "Maximum case extent rounded to the union alignment."]
            pub const RECORD_SIZE: usize = {
                let mut extent = 0;
                #(if <#records as #facade::RecordLayout>::EXTENT > extent {
                    extent = <#records as #facade::RecordLayout>::EXTENT;
                })*
                let extent = #facade::inferred_align_up(extent, Self::RECORD_ALIGNMENT);
                assert!(extent != usize::MAX, "union inferred layout overflows usize");
                extent
            };
            #[doc = "Opaque union storage metadata; case records retain their own specs."]
            pub const SPEC: #facade::RecordSpec = #facade::RecordSpec::named(
                stringify!(#name), Self::RECORD_SIZE, Self::RECORD_ALIGNMENT, &[], &[]
            ).with_documentation(#metadata);
            #[doc = "First case with its record's zero semantic value."]
            pub const ZERO: Self = Self::#first(#record::ZERO);
            #[doc = "Decodes the caller-selected case, checking direct gaps and inactive tail."]
            pub fn decode_variant(bytes: &[u8], case: #case_type) -> ::core::result::Result<Self, #facade::SchemaCodecError> {
                let actual = bytes.len();
                let bytes = bytes.get(..Self::RECORD_SIZE).ok_or(#facade::SchemaCodecError::BufferTooShort { required: Self::RECORD_SIZE, actual })?;
                match case {
                    #(#case_type::#cases => {
                        let value = #records::from_bytes(bytes)?;
                        #facade::record::validate_union_padding(bytes, #records::SPEC)?;
                        ::core::result::Result::Ok(Self::#cases(value))
                    }),*
                }
            }
            #[doc = "Atomically encodes the selected case and clears all inactive bytes."]
            pub fn encode_variant(&self, bytes: &mut [u8]) -> ::core::result::Result<(), #facade::SchemaCodecError> {
                let actual = bytes.len();
                let destination = bytes.get_mut(..Self::RECORD_SIZE).ok_or(#facade::SchemaCodecError::BufferTooShort { required: Self::RECORD_SIZE, actual })?;
                let mut encoded = [0_u8; Self::RECORD_SIZE];
                match self { #(Self::#cases(value) => value.encode_into(&mut encoded)?),* }
                destination.copy_from_slice(&encoded);
                ::core::result::Result::Ok(())
            }
        }
        impl ::core::default::Default for #name {
            fn default() -> Self { Self::ZERO }
        }
        #storage
    })
}
