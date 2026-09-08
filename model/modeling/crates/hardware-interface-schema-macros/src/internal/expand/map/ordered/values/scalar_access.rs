use proc_macro2::TokenStream;
use quote::quote;

use super::{ExpansionContext, Geometry};

pub(super) fn expand(context: &ExpansionContext<'_>, geometry: &[Geometry]) -> TokenStream {
    let facade = context.facade;
    let marker = context.marker;
    let mut reads = Vec::new();
    let mut writes = Vec::new();
    for item in geometry {
        let symbol = &item.symbol;
        let definition = &item.definition;
        let placement = &item.placement;
        for instance in 0..item.count {
            let start =
                quote!(#marker::#placement.offset() + #instance * #marker::#placement.stride());
            let (shared, exclusive) = if item.count == 1 {
                (quote!(&value.#symbol), quote!(&mut value.#symbol))
            } else {
                (
                    quote!(value.#symbol.get(#instance)?),
                    quote!(value.#symbol.get_mut(#instance)?),
                )
            };
            reads.push(quote! {
                if let ::core::option::Option::Some(relative) = offset.checked_sub(#start) {
                    if relative < <#definition as #facade::RegisterLayout>::EXTENT {
                        return <#definition as #facade::RegisterLayout>::read_register(#shared, relative);
                    }
                }
            });
            writes.push(quote! {
                if let ::core::option::Option::Some(relative) = offset.checked_sub(#start) {
                    if relative < <#definition as #facade::RegisterLayout>::EXTENT {
                        return <#definition as #facade::RegisterLayout>::write_register(#exclusive, relative, raw);
                    }
                }
            });
        }
    }
    quote! {
        fn read_register(value: &Self::Value, offset: usize) -> ::core::option::Option<u64> {
            #(#reads)*
            ::core::option::Option::None
        }
        fn write_register(value: &mut Self::Value, offset: usize, raw: u64) -> ::core::option::Option<()> {
            #(#writes)*
            ::core::option::Option::None
        }
    }
}
