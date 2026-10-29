use crate::internal::syntax::OrderedMapDecl;
use proc_macro2::TokenStream;
use quote::{ToTokens, quote};

pub(in crate::internal::expand) fn expand(
    decl: &OrderedMapDecl,
    facade: &TokenStream,
) -> TokenStream {
    let marker = &decl.name;
    let parameters: Vec<_> = decl.parameters.iter().map(syn::Ident::to_string).collect();
    let alignment = decl.alignment.0;
    let extent = option(decl.extent.map(|(value, _)| value));
    let documentation = decl.documentation.metadata(facade);
    let placements = decl.placements.iter().map(|placement| {
        let name = placement.symbol.to_string();
        let path = &placement.definition;
        let target = if placement.arguments.is_some() {
            quote!(<#path as #facade::graph::DescribeSchema>::schema_definition)
        } else { quote!(<#path as #facade::RegisterLayout>::schema_definition) };
        let arguments = placement.arguments.iter().flatten().map(|expr| expression(expr, &decl.parameters, facade));
        let count = placement.count_parameter.as_ref().map_or_else(|| {
            let count = placement.count.0;
            quote!(#facade::graph::LayoutExpr::Constant(#count))
        }, |parameter| { let name = parameter.to_string(); quote!(#facade::graph::LayoutExpr::Parameter(#name)) });
        let offset = option(placement.offset.map(|(value, _)| value));
        let bank = placement.bank;
        let docs = placement.documentation.metadata(facade);
        quote!(#facade::graph::MapPlacement::new(#name, #target, ::std::vec![#(#arguments),*], #count, #offset, #bank).with_documentation(#docs))
    });
    quote! {
        #facade::__register_schema!(<#marker as #facade::graph::DescribeSchema>::schema_definition);
        impl #facade::graph::DescribeSchema for #marker {
            fn schema_definition() -> #facade::graph::Definition {
                #facade::graph::Definition::register_map::<Self>(stringify!(#marker), #documentation,
                    #facade::graph::MapDefinition::new(&[#(#parameters),*], #alignment, #extent, ::std::vec![#(#placements),*]))
            }
        }
    }
}

fn option(value: Option<usize>) -> TokenStream {
    value.map_or_else(
        || quote!(::core::option::Option::None),
        |value| quote!(::core::option::Option::Some(#value)),
    )
}

fn expression(expr: &syn::Expr, parameters: &[syn::Ident], facade: &TokenStream) -> TokenStream {
    let unsupported = || {
        let source = expr.to_token_stream().to_string();
        quote!(#facade::graph::LayoutExpr::Unsupported(#source))
    };
    match expr {
        syn::Expr::Lit(literal) => {
            if let syn::Lit::Int(value) = &literal.lit
                && let Ok(value) = value.base10_parse::<usize>()
            {
                return quote!(#facade::graph::LayoutExpr::Constant(#value));
            }
            unsupported()
        }
        syn::Expr::Path(path) if path.qself.is_none() => path
            .path
            .get_ident()
            .filter(|name| parameters.contains(name))
            .map_or_else(
                || quote!(#facade::graph::LayoutExpr::Constant(#path)),
                |name| {
                    let name = name.to_string();
                    quote!(#facade::graph::LayoutExpr::Parameter(#name))
                },
            ),
        syn::Expr::Paren(inner) => expression(&inner.expr, parameters, facade),
        syn::Expr::Binary(binary) => {
            let left = expression(&binary.left, parameters, facade);
            let right = expression(&binary.right, parameters, facade);
            let operation = match binary.op {
                syn::BinOp::Add(_) => quote!(Add),
                syn::BinOp::Sub(_) => quote!(Subtract),
                syn::BinOp::Mul(_) => quote!(Multiply),
                _ => return unsupported(),
            };
            quote!(#facade::graph::LayoutExpr::#operation(::std::boxed::Box::new(#left), ::std::boxed::Box::new(#right)))
        }
        _ => unsupported(),
    }
}
