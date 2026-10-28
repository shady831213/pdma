use crate::internal::{crate_path, syntax::UntaggedUnionDecl};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
mod external_codec;
mod layout;
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
    let codecs: Vec<_> = decl.cases.iter().map(|(_, codec)| codec).collect();
    let Some((first, first_codec)) = decl.cases.first() else {
        return Err(syn::Error::new(
            name.span(),
            "union requires at least one case",
        ));
    };
    let storage = storage::expand(decl, &facade);
    let layout = layout::expand(decl, &facade);
    let codec = decl
        .external_codec
        .then(|| external_codec::expand(decl, &facade));
    Ok(quote! {
        #docs
        #[doc = "Semantic value of an explicitly selected complete-record union."]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #visibility enum #name { #(#[doc = "Complete codec interpretation."] #cases(<#codecs as #facade::SchemaLayout>::Value)),* }
        #[doc = "Caller-selected interpretation of untagged union storage."]
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #visibility enum #case_type { #(#[doc = "Selects this complete codec interpretation."] #cases),* }
        impl #name {
            #[doc = "Maximum natural alignment of the complete cases."]
            pub const RECORD_ALIGNMENT: usize = {
                let mut alignment = 1;
                #(assert!(<#codecs as #facade::SchemaLayout>::ALIGNMENT.is_power_of_two(), "union case alignment must be a nonzero power of two");)*
                #(if <#codecs as #facade::SchemaLayout>::ALIGNMENT > alignment {
                    alignment = <#codecs as #facade::SchemaLayout>::ALIGNMENT;
                })*
                alignment
            };
            #[doc = "Maximum case extent rounded to the union alignment."]
            pub const RECORD_SIZE: usize = {
                let mut extent = 0;
                #(if <#codecs as #facade::SchemaLayout>::EXTENT > extent {
                    extent = <#codecs as #facade::SchemaLayout>::EXTENT;
                })*
                let extent = #facade::inferred_align_up(extent, Self::RECORD_ALIGNMENT);
                assert!(extent != usize::MAX, "union inferred layout overflows usize");
                extent
            };
            #[doc = "Opaque union storage metadata; case records retain their own specs."]
            pub const SPEC: #facade::RecordSpec = #facade::RecordSpec::named(
                stringify!(#name), Self::RECORD_SIZE, Self::RECORD_ALIGNMENT, &[], &[]
            ).with_documentation(#metadata);
            #[doc = "First case with its codec's zero semantic value."]
            pub const ZERO: Self = Self::#first(<#first_codec as #facade::SchemaLayout>::ZERO);
            #[doc = "Decodes the caller-selected case, checking direct gaps and inactive tail."]
            pub fn decode_variant(bytes: &[u8], case: #case_type) -> ::core::result::Result<Self, #facade::SchemaCodecError> {
                let actual = bytes.len();
                let bytes = bytes.get(..Self::RECORD_SIZE).ok_or(#facade::SchemaCodecError::BufferTooShort { required: Self::RECORD_SIZE, actual })?;
                match case {
                    #(#case_type::#cases => {
                        let extent = <#codecs as #facade::SchemaLayout>::EXTENT;
                        let child = bytes.get(..extent).ok_or(#facade::SchemaCodecError::BufferTooShort { required: extent, actual: bytes.len() })?;
                        let value = <#codecs as #facade::SchemaCodec>::decode(child)?;
                        <#codecs as #facade::SchemaLayout>::validate_padding(child)?;
                        for (offset, byte) in bytes.iter().copied().enumerate().skip(extent) {
                            if byte != 0 {
                                return ::core::result::Result::Err(#facade::SchemaCodecError::NonZeroReserved {
                                    location: #facade::SchemaMemberIdentity::new(stringify!(#name), "tail").at_byte(offset),
                                    observed: u64::from(byte),
                                });
                            }
                        }
                        ::core::result::Result::Ok(Self::#cases(value))
                    }),*
                }
            }
            #[doc = "Atomically encodes the selected case and clears all inactive bytes."]
            pub fn encode_variant(&self, bytes: &mut [u8]) -> ::core::result::Result<(), #facade::SchemaCodecError> {
                let actual = bytes.len();
                let destination = bytes.get_mut(..Self::RECORD_SIZE).ok_or(#facade::SchemaCodecError::BufferTooShort { required: Self::RECORD_SIZE, actual })?;
                let mut encoded = [0_u8; Self::RECORD_SIZE];
                match self { #(Self::#cases(value) => {
                    let extent = <#codecs as #facade::SchemaLayout>::EXTENT;
                    let actual = encoded.len();
                    let child = encoded.get_mut(..extent).ok_or(#facade::SchemaCodecError::BufferTooShort { required: extent, actual })?;
                    <#codecs as #facade::SchemaCodec>::encode(child, value)?;
                }),* }
                destination.copy_from_slice(&encoded);
                ::core::result::Result::Ok(())
            }
        }
        impl ::core::default::Default for #name {
            fn default() -> Self { Self::ZERO }
        }
        #storage
        #layout
        #codec
    })
}
