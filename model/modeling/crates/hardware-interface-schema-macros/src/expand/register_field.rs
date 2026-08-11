use proc_macro2::TokenStream;
use quote::quote;
use syn::parse2;

use crate::{crate_path, syntax::RegisterFieldDefinitionDecl};

pub(crate) fn run(input: TokenStream) -> TokenStream {
    parse2::<RegisterFieldDefinitionDecl>(input)
        .and_then(|decl| expand(&decl))
        .unwrap_or_else(syn::Error::into_compile_error)
}

fn expand(decl: &RegisterFieldDefinitionDecl) -> syn::Result<TokenStream> {
    let facade = crate_path::facade()?;
    let visibility = &decl.visibility;
    let marker = &decl.name;
    let bits = decl.bit_width.0;
    let mask = if bits == 64 {
        u64::MAX
    } else {
        (1_u64 << bits) - 1
    };
    let name = marker.to_string();
    Ok(quote! {
        #[doc = "Marker and codec for a reusable register-field definition."]
        #[derive(Clone, Copy, Debug)] #visibility struct #marker;
        impl #marker {
            #[doc = "Reusable bit-field definition."]
            pub const DEFINITION: #facade::RegisterFieldDefinition = #facade::RegisterFieldDefinition::new(#bits);
            #[doc = "Extracts this definition from the least-significant bits."]
            pub const fn extract(value: u64) -> u64 { value & #mask }
            #[doc = "Inserts this definition into the least-significant bits."]
            pub fn insert(target: &mut u64, value: u64) -> ::core::result::Result<(), #facade::RegisterCodecError> {
                if value > #mask { return Err(#facade::RegisterCodecError::FieldValueOverflow { field_name: #name, bit_width: #bits, value }); }
                *target = (*target & !#mask) | value;
                Ok(())
            }
        }
    })
}
