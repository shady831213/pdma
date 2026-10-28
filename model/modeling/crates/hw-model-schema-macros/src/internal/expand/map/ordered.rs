mod access_tree;
mod accessors;
mod collectors;
mod context;
mod emission;
mod placement;
mod validation;
mod values;

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use crate::internal::expand::register::variant;
use crate::internal::{crate_path, syntax::OrderedMapDecl};

use self::{
    collectors::leaf_collectors,
    emission::{Emission, emit},
    placement::placement_geometry,
    validation::validate,
};

use context::{ExpansionContext, Geometry};

pub(super) fn expand(decl: &OrderedMapDecl) -> syn::Result<TokenStream> {
    validate(decl)?;
    if !decl.parameters.is_empty() || decl.placements.iter().any(|p| p.arguments.is_some()) {
        return super::instance::expand(decl);
    }
    expand_fixed(decl)
}

fn expand_fixed(decl: &OrderedMapDecl) -> syn::Result<TokenStream> {
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
        .map(|item| quote!(#[doc = "Stable generated placement ID."] #item))
        .collect();
    let all_variants = variants.iter().map(|item| quote!(Self::#item)).collect();
    let context = ExpansionContext {
        facade: &facade,
        marker,
        values_name: &values_name,
        view_name: &view_name,
        view_mut_name: &view_mut_name,
        extent: &extent,
    };
    let value_items = values::expand(decl, &context, &geometry);
    let access_tree = access_tree::expand(decl, &context, &geometry)?;
    let accessor_items = accessors::expand(decl, &context, &geometry, access_tree);
    let root_collectors = leaf_collectors(decl, marker, &id_name, &facade, &geometry, true);
    let nested_collectors = leaf_collectors(decl, marker, &id_name, &facade, &geometry, false);
    let access_paths = decl
        .access
        .iter()
        .flatten()
        .map(|entry| {
            let group_names = entry.groups.iter().map(syn::Ident::to_string);
            let leaf_name = entry.leaf.to_string();
            let target = variant(&entry.target.to_string());
            quote!(#facade::RegisterAccessPath::new(
                &[#(#facade::RegisterAccessGroup::new(#group_names)),*],
                #facade::RegisterAccessLeaf::new(#leaf_name, #id_name::#target),
            ))
        })
        .collect();

    Ok(emit(Emission {
        documentation: decl.documentation.metadata(&facade),
        rust_docs: decl.documentation.rust_docs(),
        visibility,
        id_name: &id_name,
        documented_variants,
        all_variants,
        variant_count: variants.len(),
        marker,
        marker_name,
        internals,
        alignment,
        extent,
        facade: &facade,
        placements,
        root_collectors,
        nested_collectors,
        assertions,
        value_items,
        accessor_items,
        access_paths,
    }))
}
