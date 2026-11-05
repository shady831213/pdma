use proc_macro2::TokenStream;
use quote::quote;

pub(super) struct Emission<'a> {
    pub(super) documentation: TokenStream,
    pub(super) rust_docs: TokenStream,
    pub(super) visibility: &'a syn::Visibility,
    pub(super) id_name: &'a syn::Ident,
    pub(super) documented_variants: Vec<TokenStream>,
    pub(super) all_variants: Vec<TokenStream>,
    pub(super) variant_count: usize,
    pub(super) marker: &'a syn::Ident,
    pub(super) marker_name: String,
    pub(super) internals: Vec<TokenStream>,
    pub(super) alignment: usize,
    pub(super) extent: TokenStream,
    pub(super) facade: &'a TokenStream,
    pub(super) placements: Vec<TokenStream>,
    pub(super) root_collectors: TokenStream,
    pub(super) nested_collectors: TokenStream,
    pub(super) assertions: Vec<TokenStream>,
    pub(super) value_items: TokenStream,
    pub(super) accessor_items: TokenStream,
    pub(super) access_paths: Vec<TokenStream>,
}

pub(super) fn emit(input: Emission<'_>) -> TokenStream {
    let Emission {
        documentation,
        rust_docs,
        visibility,
        id_name,
        documented_variants,
        all_variants,
        variant_count,
        marker,
        marker_name,
        internals,
        alignment,
        extent,
        facade,
        placements,
        root_collectors,
        nested_collectors,
        assertions,
        value_items,
        accessor_items,
        access_paths,
    } = input;
    quote! {
        #[doc = "Stable identities for ordered map placements."]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        #visibility enum #id_name { #(#documented_variants),* }
        impl #id_name {
            #[doc = "Every generated top-level placement identity in declaration order."]
            pub const ALL: [Self; #variant_count] = [#(#all_variants),*];
        }
        #rust_docs
        #[doc = "Marker and metadata namespace for the generated ordered register map."]
        #[derive(Clone, Copy, Debug)]
        #visibility struct #marker;
        impl #marker {
            #[doc = "Declared schema documentation."] pub const DOCUMENTATION: #facade::Documentation = #documentation;
            #(#internals)*
            #[doc = "Required byte alignment for this map."] pub const ALIGNMENT: usize = #alignment;
            #[doc = "Exact encoded byte extent for this map."] pub const EXTENT: usize = #extent;
            const __PLACEMENTS: &'static [#facade::RegisterMapPlacement<#id_name>] = &[#(#placements),*];
            #[doc = "Ordered child-layout metadata."] pub const SPEC: #facade::OrderedRegisterMapSpec<#id_name> = #facade::OrderedRegisterMapSpec::new(#marker_name, #alignment, #extent, Self::__PLACEMENTS).with_documentation(Self::DOCUMENTATION);
            #[doc = "Explicit access-only paths in declaration order."] pub const ACCESS_PATHS: &'static [#facade::RegisterAccessPath<#id_name>] = &[#(#access_paths),*];
            #[doc = "Resolves every recursively placed leaf register."]
            pub fn resolve() -> ::core::result::Result<#facade::ResolvedRegisterMap<#id_name>, #facade::RegisterMapError<#id_name>> {
                Self::__resolved().clone()
            }
            #[doc = "Returns the first leaf carrying one top-level placement identity."]
            pub fn register(id: #id_name) -> ::core::option::Option<#facade::register::RegisterSpec<#id_name>> {
                let resolved = Self::__resolved().as_ref().ok()?;
                let leaf = resolved.registers().iter().find(|leaf| leaf.id() == id)?;
                ::core::option::Option::Some(#facade::register::RegisterSpec::place(leaf.id(), leaf.name(), leaf.offset(), leaf.definition()).with_documentation(leaf.documentation()))
            }
            #[doc = "Locates a leaf register only at its exact expanded start."]
            pub fn locate(offset: usize) -> ::core::option::Option<#facade::RegisterLocation<#id_name>> {
                let resolved = Self::__resolved().as_ref().ok()?;
                let leaf = resolved.locate(offset)?;
                let register = #facade::register::RegisterSpec::place(leaf.id(), leaf.name(), leaf.offset(), leaf.definition()).with_documentation(leaf.documentation());
                match (leaf.bank_name(), leaf.instance()) {
                    (::core::option::Option::Some(bank_name), ::core::option::Option::Some(instance)) => ::core::option::Option::Some(#facade::RegisterLocation::Repeated { bank_name, instance, register, offset }),
                    _ => ::core::option::Option::Some(#facade::RegisterLocation::Fixed { register, offset }),
                }
            }
            fn __resolved() -> &'static ::core::result::Result<#facade::ResolvedRegisterMap<#id_name>, #facade::RegisterMapError<#id_name>> {
                static RESOLVED: ::std::sync::OnceLock<::core::result::Result<#facade::ResolvedRegisterMap<#id_name>, #facade::RegisterMapError<#id_name>>> = ::std::sync::OnceLock::new();
                RESOLVED.get_or_init(|| {
                    let mut registers = ::std::vec::Vec::new();
                    let mut placements = ::std::vec::Vec::new();
                    let mut declaration_index = 0usize;
                    #root_collectors
                    #facade::finish_ordered_register_map(#marker_name, Self::EXTENT, registers).and_then(|map| map.with_documentation(Self::DOCUMENTATION).with_placements(placements))
                })
            }
            fn __append_registers<I: ::core::marker::Copy + 'static>(collection: #facade::RegisterCollection<'_, I>) {
                let #facade::RegisterCollection { id, path, offset, declaration_index, repeated, output, placements, .. } = collection;
                #nested_collectors
            }
            fn __placement_range(
                placement: #facade::RegisterMapPlacement<#id_name>,
                instance: usize,
            ) -> ::core::result::Result<::core::ops::Range<usize>, #facade::SchemaCodecError> {
                let ::core::option::Option::Some(step) = instance.checked_mul(placement.stride()) else {
                    return ::core::result::Result::Err(#facade::SchemaCodecError::BufferTooShort { required: Self::EXTENT, actual: 0 });
                };
                let ::core::option::Option::Some(start) = placement.offset().checked_add(step) else {
                    return ::core::result::Result::Err(#facade::SchemaCodecError::BufferTooShort { required: Self::EXTENT, actual: 0 });
                };
                let ::core::option::Option::Some(end) = start.checked_add(placement.extent()) else {
                    return ::core::result::Result::Err(#facade::SchemaCodecError::BufferTooShort { required: Self::EXTENT, actual: 0 });
                };
                ::core::result::Result::Ok(start..end)
            }
        }
        #(#assertions)*
        #value_items
        #accessor_items
    }
}
