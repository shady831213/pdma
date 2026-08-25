mod accessors;
mod values;

use std::collections::HashSet;

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::internal::{
    crate_path,
    expand::register::variant,
    syntax::{OrderedMapDecl, OrderedPlacementDecl},
};

pub(super) fn expand(decl: &OrderedMapDecl) -> syn::Result<TokenStream> {
    validate(decl)?;
    let facade = crate_path::facade()?;
    let visibility = &decl.visibility;
    let marker = &decl.name;
    let marker_name = marker.to_string();
    let id_name = format_ident!("{}RegisterId", marker);
    let view_name = format_ident!("{}View", marker);
    let view_mut_name = format_ident!("{}ViewMut", marker);
    let values_name = format_ident!("{}Values", marker);
    let alignment = decl.alignment.0;
    let mut cursor = quote!(0usize);
    let mut internals = Vec::new();
    let mut assertions = vec![quote!(
        const _: () = assert!(#alignment != 0 && #alignment.is_power_of_two(), "register map alignment must be a nonzero power of two");
    )];
    let mut variants = Vec::new();
    let mut placements = Vec::new();
    let mut prior_instances: Vec<(TokenStream, TokenStream)> = Vec::new();
    let mut geometry = Vec::new();

    for placement in &decl.placements {
        let tokens = placement_geometry(placement, marker, &id_name, &facade, alignment, &cursor);
        cursor = tokens.high_water.clone();
        internals.extend(tokens.internals);
        assertions.extend(tokens.assertions);
        for current in &tokens.instances {
            for prior in &prior_instances {
                let (current_start, current_end) = current;
                let (prior_start, prior_end) = prior;
                assertions.push(quote!(const _: () = assert!(#current_start >= #prior_end || #prior_start >= #current_end, "register map placements overlap");));
            }
        }
        prior_instances.extend(tokens.instances);
        variants.push(tokens.item_variant);
        placements.push(tokens.spec);
        geometry.push(tokens.geometry);
    }

    let extent = decl.extent.map_or_else(
        || cursor.clone(),
        |extent| {
            let value = extent.0;
            quote!(#value)
        },
    );
    assertions.push(quote!(const _: () = assert!(#cursor <= #extent, "register map placement extends beyond its explicit extent");));
    let documented_variants = variants
        .iter()
        .map(|item| quote!(#[doc = "Stable generated placement ID."] #item));
    let context = ExpansionContext {
        facade: &facade,
        marker,
        values_name: &values_name,
        view_name: &view_name,
        view_mut_name: &view_mut_name,
        extent: &extent,
    };
    let value_items = values::expand(decl, &context, &geometry);
    let accessor_items = accessors::expand(decl, &context, &geometry);

    Ok(quote! {
        #[doc = "Stable identities for ordered map placements."]
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        #visibility enum #id_name { #(#documented_variants),* }
        #[doc = "Marker and metadata namespace for the generated ordered register map."]
        #[derive(Clone, Copy, Debug)]
        #visibility struct #marker;
        impl #marker {
            #(#internals)*
            #[doc = "Required byte alignment for this map."] pub const ALIGNMENT: usize = #alignment;
            #[doc = "Exact encoded byte extent for this map."] pub const EXTENT: usize = #extent;
            const __PLACEMENTS: &'static [#facade::RegisterMapPlacement<#id_name>] = &[#(#placements),*];
            #[doc = "Ordered child-layout metadata."] pub const SPEC: #facade::OrderedRegisterMapSpec<#id_name> = #facade::OrderedRegisterMapSpec::new(#marker_name, #alignment, #extent, Self::__PLACEMENTS);
            fn __placement_range(
                placement: #facade::RegisterMapPlacement<#id_name>,
                instance: usize,
            ) -> ::core::result::Result<::core::ops::Range<usize>, #facade::SchemaCodecError> {
                let Some(step) = instance.checked_mul(placement.stride()) else {
                    return Err(#facade::SchemaCodecError::BufferTooShort { required: Self::EXTENT, actual: 0 });
                };
                let Some(start) = placement.offset().checked_add(step) else {
                    return Err(#facade::SchemaCodecError::BufferTooShort { required: Self::EXTENT, actual: 0 });
                };
                let Some(end) = start.checked_add(placement.extent()) else {
                    return Err(#facade::SchemaCodecError::BufferTooShort { required: Self::EXTENT, actual: 0 });
                };
                Ok(start..end)
            }
        }
        #(#assertions)*
        #value_items
        #accessor_items
    })
}

pub(super) struct Geometry {
    pub(super) symbol: syn::Ident,
    pub(super) definition: syn::Path,
    pub(super) placement: syn::Ident,
    pub(super) count: usize,
}

pub(super) struct ExpansionContext<'a> {
    pub(super) facade: &'a TokenStream,
    pub(super) marker: &'a syn::Ident,
    pub(super) values_name: &'a syn::Ident,
    pub(super) view_name: &'a syn::Ident,
    pub(super) view_mut_name: &'a syn::Ident,
    pub(super) extent: &'a TokenStream,
}

struct PlacementTokens {
    item_variant: syn::Ident,
    internals: Vec<TokenStream>,
    assertions: Vec<TokenStream>,
    instances: Vec<(TokenStream, TokenStream)>,
    spec: TokenStream,
    high_water: TokenStream,
    geometry: Geometry,
}

fn placement_geometry(
    placement: &OrderedPlacementDecl,
    marker: &syn::Ident,
    id_name: &syn::Ident,
    facade: &TokenStream,
    map_alignment: usize,
    cursor: &TokenStream,
) -> PlacementTokens {
    let symbol = &placement.symbol;
    let definition = &placement.definition;
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
        |value| { let value = value.0; quote!(#value) },
    );
    let stride = placement.stride.map_or_else(
        || quote!(match #facade::checked_align_up(<#definition as #facade::RegisterLayout>::EXTENT, <#definition as #facade::RegisterLayout>::ALIGNMENT) { Some(value) => value, None => 0 }),
        |value| { let value = value.0; quote!(#value) },
    );
    let id = quote!(#id_name::#item_variant);
    let name = symbol.to_string();
    let spec = quote!(#facade::RegisterMapPlacement::new(#id, #name, Self::#offset_name, #count, Self::#stride_name, #facade::RegisterLayoutGeometry::new(<#definition as #facade::RegisterLayout>::ALIGNMENT, <#definition as #facade::RegisterLayout>::EXTENT)));
    let internals = vec![
        quote!(const #offset_name: usize = #offset;),
        quote!(const #stride_name: usize = #stride;),
        quote!(const #end_name: usize = match #facade::checked_placement_end(Self::#offset_name, #count, Self::#stride_name, <#definition as #facade::RegisterLayout>::EXTENT) { Some(value) => value, None => 0 };),
        quote!(const #high_water_name: usize = if #cursor > Self::#end_name { #cursor } else { Self::#end_name };),
        quote!(#[doc = "Generated ordered child placement."] pub const #constant: #facade::RegisterMapPlacement<#id_name> = #spec;),
        quote!(#[doc = "Stable generated placement member identity."] pub const #identity_name: #facade::SchemaMemberIdentity = #facade::SchemaMemberIdentity::new(stringify!(#marker), #name);),
    ];
    let alignment_check =
        quote!(#map_alignment % <#definition as #facade::RegisterLayout>::ALIGNMENT == 0);
    let offset_alignment =
        quote!(#marker::#offset_name % <#definition as #facade::RegisterLayout>::ALIGNMENT == 0);
    let stride_alignment = quote!(#marker::#stride_name >= <#definition as #facade::RegisterLayout>::EXTENT && #marker::#stride_name % <#definition as #facade::RegisterLayout>::ALIGNMENT == 0);
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
            symbol: placement.symbol.clone(),
            definition: placement.definition.clone(),
            placement: constant,
            count,
        },
    }
}

fn validate(decl: &OrderedMapDecl) -> syn::Result<()> {
    if decl.alignment.0 == 0 || !decl.alignment.0.is_power_of_two() {
        return Err(syn::Error::new(
            decl.alignment.1,
            "register map alignment must be a nonzero power of two",
        ));
    }
    let mut symbols = HashSet::new();
    for placement in &decl.placements {
        if !symbols.insert(placement.symbol.to_string()) {
            return Err(syn::Error::new(
                placement.symbol.span(),
                "duplicate register map placement name",
            ));
        }
        if placement.count.0 == 0 {
            return Err(syn::Error::new(
                placement.count.1,
                "register map placement count must be nonzero",
            ));
        }
        if placement.count.0 == 1 && placement.stride.is_some() {
            return Err(syn::Error::new(
                placement.stride.map_or(placement.count.1, |value| value.1),
                "stride is not valid for a singleton placement",
            ));
        }
    }
    Ok(())
}
