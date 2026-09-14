use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::internal::{expand::register::variant, syntax::OrderedPlacementDecl};

use super::Geometry;

pub(super) struct PlacementTokens {
    pub(super) item_variant: syn::Ident,
    pub(super) internals: Vec<TokenStream>,
    pub(super) assertions: Vec<TokenStream>,
    pub(super) instances: Vec<(TokenStream, TokenStream)>,
    pub(super) spec: TokenStream,
    pub(super) high_water: TokenStream,
    pub(super) geometry: Geometry,
}

pub(super) fn placement_geometry(
    placement: &OrderedPlacementDecl,
    marker: &syn::Ident,
    id_name: &syn::Ident,
    facade: &TokenStream,
    map_alignment: usize,
    cursor: &TokenStream,
) -> PlacementTokens {
    let symbol = &placement.symbol;
    let definition = &placement.definition;
    let documentation = placement.documentation.metadata(facade);
    let rust_docs = placement.documentation.rust_docs();
    let item_variant = variant(&symbol.to_string());
    let constant = format_ident!("{}", symbol.to_string().to_uppercase());
    let identity_name = format_ident!("{}_IDENTITY", constant);
    let offset_name = format_ident!("__{}_OFFSET", constant);
    let stride_name = format_ident!("__{}_STRIDE", constant);
    let end_name = format_ident!("__{}_END", constant);
    let high_water_name = format_ident!("__{}_HIGH_WATER", constant);
    let count = placement.count.0;
    let offset = placement.offset.map_or_else(
        || quote!(match #facade::checked_align_up(#cursor, <#definition as #facade::RegisterLayout>::ALIGNMENT) { Some(value) => value, None => 0 }),
        |value| {
            let value = value.0;
            quote!(#value)
        },
    );
    let stride = placement.stride.map_or_else(
        || quote!(match #facade::checked_align_up(<#definition as #facade::RegisterLayout>::EXTENT, <#definition as #facade::RegisterLayout>::ALIGNMENT) { Some(value) => value, None => 0 }),
        |value| {
            let value = value.0;
            quote!(#value)
        },
    );
    let id = quote!(#id_name::#item_variant);
    let name = symbol.to_string();
    let spec = quote!(#facade::RegisterMapPlacement::new(#id, #name, Self::#offset_name, #count, Self::#stride_name, #facade::RegisterLayoutGeometry::new(<#definition as #facade::RegisterLayout>::ALIGNMENT, <#definition as #facade::RegisterLayout>::EXTENT)).with_documentation((#documentation).with_fallback(<#definition as #facade::RegisterLayout>::DOCUMENTATION)));
    let internals = vec![
        quote!(const #offset_name: usize = #offset;),
        quote!(const #stride_name: usize = #stride;),
        quote!(const #end_name: usize = match #facade::checked_placement_end(Self::#offset_name, #count, Self::#stride_name, <#definition as #facade::RegisterLayout>::EXTENT) { Some(value) => value, None => 0 };),
        quote!(const #high_water_name: usize = if #cursor > Self::#end_name { #cursor } else { Self::#end_name };),
        quote!(#rust_docs #[doc = "Generated ordered child placement."] pub const #constant: #facade::RegisterMapPlacement<#id_name> = #spec;),
        quote!(#[doc = "Stable generated placement member identity."] pub const #identity_name: #facade::SchemaMemberIdentity = #facade::SchemaMemberIdentity::new(stringify!(#marker), #name);),
    ];
    let alignment_mask =
        quote!(<#definition as #facade::RegisterLayout>::ALIGNMENT.saturating_sub(1));
    let alignment_check = quote!(#map_alignment & #alignment_mask == 0);
    let offset_alignment = quote!(
        matches!(#facade::checked_align_up(#marker::#offset_name, <#definition as #facade::RegisterLayout>::ALIGNMENT), Some(aligned) if aligned == #marker::#offset_name)
    );
    let stride_alignment = quote!(#marker::#stride_name >= <#definition as #facade::RegisterLayout>::EXTENT && #marker::#stride_name & #alignment_mask == 0);
    let mut assertions = vec![
        quote!(
            const _: () = assert!(<#definition as #facade::RegisterLayout>::ALIGNMENT != 0 && <#definition as #facade::RegisterLayout>::ALIGNMENT.is_power_of_two(), "child layout alignment must be a nonzero power of two");
        ),
        quote!(
            const _: () = assert!(#count > 0, "register map placement count must be nonzero");
        ),
        quote!(
            const _: () = assert!(#alignment_check, "child layout alignment exceeds the containing map alignment");
        ),
        quote!(
            const _: () = assert!(#offset_alignment, "register map placement offset is misaligned");
        ),
        quote!(
            const _: () = assert!(#stride_alignment, "register map placement stride is invalid");
        ),
        quote!(
            const _: () = assert!(#facade::checked_placement_end(#marker::#offset_name, #count, #marker::#stride_name, <#definition as #facade::RegisterLayout>::EXTENT).is_some(), "register map placement arithmetic overflow");
        ),
    ];
    if placement.offset.is_none() {
        assertions.push(quote!(const _: () = assert!(#facade::checked_align_up(#cursor, <#definition as #facade::RegisterLayout>::ALIGNMENT).is_some(), "register map implicit offset overflows usize");));
    }
    if placement.stride.is_none() {
        assertions.push(quote!(const _: () = assert!(#facade::checked_align_up(<#definition as #facade::RegisterLayout>::EXTENT, <#definition as #facade::RegisterLayout>::ALIGNMENT).is_some(), "register map implicit stride overflows usize");));
    }
    if count == 1 && placement.stride.is_some() {
        assertions.push(quote!(
            const _: () = assert!(false, "stride is not valid for a singleton placement");
        ));
    }
    let instances = (0..count)
        .map(|instance| {
            let start = quote!(#marker::#offset_name + #instance * #marker::#stride_name);
            let end = quote!(#start + <#definition as #facade::RegisterLayout>::EXTENT);
            (start, end)
        })
        .collect();
    PlacementTokens {
        item_variant,
        internals,
        assertions,
        instances,
        spec,
        high_water: quote!(#marker::#high_water_name),
        geometry: Geometry {
            rust_docs,
            symbol: placement.symbol.clone(),
            definition: placement.definition.clone(),
            placement: constant,
            count,
        },
    }
}
