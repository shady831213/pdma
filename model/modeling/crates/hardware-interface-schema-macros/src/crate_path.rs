use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

pub(crate) fn facade() -> syn::Result<TokenStream> {
    match crate_name("hardware-interface-schema") {
        Ok(FoundCrate::Itself) => Ok(quote!(crate)),
        Ok(FoundCrate::Name(name)) => {
            let ident = format_ident!("{name}");
            Ok(quote!(::#ident))
        }
        Err(error) => Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            format!("cannot resolve hardware-interface-schema facade: {error}"),
        )),
    }
}
