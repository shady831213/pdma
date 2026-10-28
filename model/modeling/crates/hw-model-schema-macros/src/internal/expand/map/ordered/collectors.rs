use proc_macro2::TokenStream;
use quote::quote;

use crate::internal::{expand::register::variant, syntax::OrderedMapDecl};

use super::Geometry;

pub(super) fn leaf_collectors(
    decl: &OrderedMapDecl,
    marker: &syn::Ident,
    id_name: &syn::Ident,
    facade: &TokenStream,
    geometry: &[Geometry],
    root: bool,
) -> TokenStream {
    let marker_name = marker.to_string();
    let calls = decl
        .placements
        .iter()
        .zip(geometry)
        .map(|(placement, geometry)| {
            let definition = &geometry.definition;
            let placement_constant = &geometry.placement;
            let symbol = placement.symbol.to_string();
            let register_name = symbol.to_ascii_uppercase();
            let item_variant = variant(&symbol);
            let id = if root { quote!(#id_name::#item_variant) } else { quote!(id) };
            let parent_path = if root {
                quote!(::std::string::String::from(#marker_name))
            } else {
                quote!(path.clone())
            };
            let base = if root { quote!(0usize) } else { quote!(offset) };
            let declaration_index = if root {
                quote!(&mut declaration_index)
            } else {
                quote!(declaration_index)
            };
            let output = if root { quote!(&mut registers) } else { quote!(output) };
            let placements = if root { quote!(&mut placements) } else { quote!(placements) };
            let bank_node = geometry.bank.then(|| quote! {
                (#placements).push(#facade::ResolvedRegisterMapPlacement::new(
                    ::std::format!("{}.{}", #parent_path, #symbol),
                    #facade::RegisterMapPlacement::new(#id, #symbol, #base + #marker::#placement_constant.offset(), #marker::#placement_constant.count(), #marker::#placement_constant.stride(),
                        #facade::RegisterLayoutGeometry::new(<#definition as #facade::RegisterLayout>::ALIGNMENT, <#definition as #facade::RegisterLayout>::EXTENT))
                        .as_bank().with_documentation(#marker::#placement_constant.documentation()),
                ));
            });
            let instances = (0..geometry.count)
                .map(move |instance| {
                    let child_path = if geometry.bank {
                        quote!(::std::format!("{}.{}[{}]", #parent_path, #symbol, #instance))
                    } else {
                        quote!(::std::format!("{}.{}", #parent_path, #symbol))
                    };
                    let repeated = if geometry.bank {
                        quote!(::core::option::Option::Some((#symbol, #instance)))
                    } else if root { quote!(::core::option::Option::None) } else { quote!(repeated) };
                    quote! {
                        <#definition as #facade::RegisterLayout>::append_registers(#facade::RegisterCollection {
                            documentation: #marker::#placement_constant.documentation(),
                            placements: #placements,
                            id: #id,
                            name: #register_name,
                            path: #child_path,
                            offset: #base + #marker::#placement_constant.offset() + #instance * #marker::#placement_constant.stride(),
                            declaration_index: #declaration_index,
                            repeated: #repeated,
                            output: #output,
                        });
                    }
                })
                .collect::<Vec<_>>();
            quote!(#bank_node #(#instances)*)
        });
    quote!(#(#calls)*)
}
