mod collect;
mod placement;

use crate::internal::expand::register::variant;
use crate::internal::{crate_path, syntax::OrderedMapDecl};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};

pub(super) fn expand(decl: &OrderedMapDecl) -> syn::Result<TokenStream> {
    let cursor = local("cursor");
    let facade = crate_path::facade()?;
    let marker = &decl.name;
    let layout = format_ident!("{}Layout", marker);
    let id = format_ident!("{}RegisterId", marker);
    let visibility = &decl.visibility;
    let alignment = decl.alignment.0;
    let documentation = decl.documentation.metadata(&facade);
    let parameters = &decl.parameters;
    let symbols: Vec<_> = decl.placements.iter().map(|p| &p.symbol).collect();
    let variants: Vec<_> = symbols.iter().map(|s| variant(&s.to_string())).collect();
    let bindings: Vec<_> = symbols
        .iter()
        .map(|s| local(&format!("__placement_{s}")))
        .collect();
    let mut fields = Vec::new();
    let mut initializers = Vec::new();
    let mut accessors = Vec::new();
    let mut constants = Vec::new();
    let mut prefix = Some(quote!(0usize));
    for p in &decl.placements {
        let tokens = placement::expand(p, &facade, &id, &mut prefix);
        fields.push(tokens.field);
        initializers.push(tokens.initialize);
        accessors.push(tokens.accessor);
        constants.extend(tokens.constants);
    }
    let checks = overlap_checks(&bindings);
    let extent = decl
        .extent
        .map_or_else(|| quote!(#cursor), |(value, _)| quote!(#value));
    let collectors = collect::expand(decl, &facade, &id, true);
    let nested = collect::expand(decl, &facade, &id, false);
    Ok(quote! {
        #[doc = "Stable placement identities."]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #visibility enum #id { #(#[doc = "Generated placement identity."] #variants),* }
        #[doc = "Constructor-bound register-map schema."]
        #[derive(Debug, Clone, Copy)]
        #visibility struct #marker;
        #[doc = "Immutable register-map instance geometry, without hardware values."]
        #[derive(Debug, Clone)]
        #visibility struct #layout { extent: usize, #(#fields),* }
        impl #marker {
            #[doc = "Required map alignment."] pub const ALIGNMENT: usize = #alignment;
            #[doc = "Schema documentation."] pub const DOCUMENTATION: #facade::Documentation = #documentation;
            #(#constants)*
            #[doc = "Binds and validates immutable map parameters."]
            pub fn new(#(#parameters: usize),*) -> Result<#layout, #facade::RegisterBankError> {
                let mut #cursor = 0usize;
                #(#initializers)*
                #(#checks)*
                let extent = #extent;
                if #cursor > extent { return Err(#facade::RegisterBankError::Extent); }
                Ok(#layout { extent, #(#symbols: #bindings),* })
            }
        }
        impl #layout {
            #[doc = "Returns the map extent including internal holes."]
            pub const fn extent(&self) -> usize { self.extent }
            #(#accessors)*
            #[doc = "Returns the terminal miss after all occupied siblings were tried."]
            pub fn finish_access<E>(&self, address: usize) -> #facade::RegisterAccessError<E> {
                if address < self.extent { #facade::RegisterAccessError::InvalidOffset { offset: address } }
                else { #facade::RegisterAccessError::OutsideWindow { address } }
            }
            #[doc = "Resolves the generated leaf metadata for this instance."]
            pub fn resolve(&self) -> Result<#facade::ResolvedRegisterMap<#id>, #facade::RegisterMapError<#id>> {
                let mut registers = Vec::new();
                let mut placements = Vec::new();
                let mut declaration_index = 0usize;
                #collectors
                #facade::finish_ordered_register_map(stringify!(#marker), self.extent, registers)
                    .map(|map| map.with_documentation(#marker::DOCUMENTATION).with_placements(placements))
            }
        }
        impl #facade::RegisterShape for #layout {
            fn documentation(&self) -> #facade::Documentation { #marker::DOCUMENTATION }
            fn alignment(&self) -> usize { #alignment }
            fn extent(&self) -> usize { self.extent }
            fn append_registers<I: Copy + 'static>(&self, collection: #facade::RegisterCollection<'_, I>) {
                collection.placements.push(#facade::ResolvedRegisterMapPlacement::new(
                    collection.path.clone(),
                    #facade::RegisterMapPlacement::new(collection.id, collection.name, collection.offset, 1, 0,
                        #facade::RegisterLayoutGeometry::new(#alignment, self.extent))
                        .with_documentation(collection.documentation.with_fallback(#marker::DOCUMENTATION)),
                ));
                let #facade::RegisterCollection { id, path, offset, declaration_index, repeated, output, placements, .. } = collection;
                #nested
            }
        }
    })
}

fn local(name: &str) -> syn::Ident {
    syn::Ident::new(name, proc_macro2::Span::mixed_site())
}

fn overlap_checks(bindings: &[syn::Ident]) -> Vec<TokenStream> {
    let mut checks = Vec::new();
    for (index, current) in bindings.iter().enumerate() {
        for previous in bindings.iter().take(index) {
            checks.push(quote!(#current.validate_disjoint(&#previous)?;));
        }
    }
    checks
}
