use crate::internal::{expand::register::variant, syntax::OrderedMapDecl};
use proc_macro2::TokenStream;
use quote::quote;

pub(super) fn expand(
    decl: &OrderedMapDecl,
    facade: &TokenStream,
    id_type: &syn::Ident,
    root: bool,
) -> TokenStream {
    let marker = &decl.name;
    let calls = decl.placements.iter().map(|p| {
        let symbol = &p.symbol;
        let name = symbol.to_string();
        let register_name = name.to_ascii_uppercase();
        let variant = variant(&name);
        let documentation = p.documentation.metadata(facade);
        let parent = if root { quote!(stringify!(#marker)) } else { quote!(path) };
        let base = if root { quote!(0usize) } else { quote!(offset) };
        let id = if root { quote!(#id_type::#variant) } else { quote!(id) };
        let child_path = if p.bank { quote!(format!("{}.{}[{}]", #parent, #name, instance)) } else { quote!(format!("{}.{}", #parent, #name)) };
        let repeated = if p.bank { quote!(Some((#name, instance))) } else if root { quote!(None) } else { quote!(repeated) };
        let output = if root { quote!(&mut registers) } else { quote!(output) };
        let placements = if root { quote!(&mut placements) } else { quote!(placements) };
        let index = if root { quote!(&mut declaration_index) } else { quote!(declaration_index) };
        let bank_node = p.bank.then(|| quote! {
            (#placements).push(#facade::ResolvedRegisterMapPlacement::new(
                format!("{}.{}", #parent, #name),
                #facade::RegisterMapPlacement::new(#id, #name, #base + self.#symbol.base(), self.#symbol.count(), self.#symbol.stride(),
                    #facade::RegisterLayoutGeometry::new(#facade::RegisterShape::alignment(self.#symbol.child()), #facade::RegisterShape::extent(self.#symbol.child())))
                    .as_bank().with_documentation((#documentation).with_fallback(#facade::RegisterShape::documentation(self.#symbol.child()))),
            ));
        });
        quote! {
            #bank_node
            for instance in 0..self.#symbol.count() {
                #facade::RegisterShape::append_registers(self.#symbol.child(), #facade::RegisterCollection {
                    documentation: #documentation,
                    placements: #placements,
                    id: #id,
                    name: #register_name,
                    path: #child_path,
                    offset: #base + self.#symbol.base() + instance * self.#symbol.stride(),
                    declaration_index: #index,
                    repeated: #repeated,
                    output: #output,
                });
            }
        }
    });
    quote!(#(#calls)*)
}
