use crate::internal::{expand::register::variant, syntax::OrderedPlacementDecl};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::ext::IdentExt;

pub(super) struct Tokens {
    pub field: TokenStream,
    pub initialize: TokenStream,
    pub accessor: TokenStream,
    pub constants: Vec<TokenStream>,
}

pub(super) fn expand(
    p: &OrderedPlacementDecl,
    facade: &TokenStream,
    id: &syn::Ident,
    prefix: &mut Option<TokenStream>,
) -> Tokens {
    let symbol = &p.symbol;
    let binding = super::local(&format!("__placement_{}", symbol.unraw()));
    let cursor = super::local("cursor");
    let child = super::local("child");
    let child_alignment = super::local("alignment");
    let child_offset = super::local("offset");
    let definition = &p.definition;
    let visibility = &p.visibility;
    let (child_type, child_value) = p.arguments.as_ref().map_or_else(
        || {
            (
                quote!(#facade::StaticRegisterLayout<#definition>),
                quote!(#facade::StaticRegisterLayout::<#definition>::new()),
            )
        },
        |arguments| {
            let mut layout = definition.clone();
            if let Some(last) = layout.segments.last_mut() {
                last.ident = format_ident!("{}Layout", last.ident);
            }
            (quote!(#layout), quote!(#definition::new(#(#arguments),*)?))
        },
    );
    let count = p.count_parameter.as_ref().map_or_else(
        || {
            let n = p.count.0;
            quote!(#n)
        },
        |n| quote!(#n),
    );
    let offset = p.offset.map_or_else(
        || quote!(#facade::checked_align_up(#cursor, #facade::RegisterShape::alignment(&#child)).ok_or(#facade::RegisterBankError::Overflow)?),
        |(n, _)| quote!(#n),
    );
    let constant = format_ident!("{}", symbol.unraw().to_string().to_uppercase());
    let offset_name = format_ident!("{}_OFFSET", constant);
    let stride_name = format_ident!("{}_STRIDE", constant);
    let mut constants = Vec::new();
    let static_offset = p.offset.map(|(n, _)| quote!(#n)).or_else(|| prefix.as_ref().filter(|_| p.arguments.is_none()).map(|cursor| quote!(#facade::inferred_align_up(#cursor, <#definition as #facade::RegisterLayout>::ALIGNMENT))));
    let static_stride = p.arguments.is_none().then(|| quote!(#facade::inferred_align_up(<#definition as #facade::RegisterLayout>::EXTENT, <#definition as #facade::RegisterLayout>::ALIGNMENT)));
    if let Some(offset) = &static_offset {
        constants.push(quote!(#[doc = "Static first-instance offset."] pub const #offset_name: usize = #offset;));
    }
    if let Some(stride) = &static_stride {
        constants.push(quote!(#[doc = "Static child-instance stride."] pub const #stride_name: usize = #stride;));
    }
    if let (Some(offset), Some(stride)) = (&static_offset, &static_stride) {
        if p.arguments.is_none() && p.count_parameter.is_none() {
            let variant = variant(symbol);
            let name = symbol.to_string();
            constants.push(quote!(#[doc = "Fixed placement metadata."] pub const #constant: #facade::RegisterMapPlacement<#id> = #facade::RegisterMapPlacement::new(#id::#variant, #name, #offset, #count, #stride, #facade::RegisterLayoutGeometry::new(<#definition as #facade::RegisterLayout>::ALIGNMENT, <#definition as #facade::RegisterLayout>::EXTENT));));
            let end = quote!(match #facade::checked_placement_end(#offset, #count, #stride, <#definition as #facade::RegisterLayout>::EXTENT) { ::core::option::Option::Some(end) => end, ::core::option::Option::None => usize::MAX });
            *prefix = prefix.as_ref().map(|prior| quote!({ let end = #end; let prior = #prior; if prior > end { prior } else { end } }));
        } else {
            *prefix = None;
        }
    } else {
        *prefix = None;
    }
    Tokens {
        field: quote!(#symbol: #facade::RegisterBank<#child_type>),
        initialize: quote! {
            let #binding = {
                let #child = #child_value;
                let #child_alignment = #facade::RegisterShape::alignment(&#child);
                if #child_alignment == 0 || Self::ALIGNMENT % #child_alignment != 0 { return ::core::result::Result::Err(#facade::RegisterBankError::Alignment); }
                let #child_offset = #offset;
                #facade::RegisterBank::new(#child, #child_offset, #count)?
            };
            #cursor = #cursor.max(#binding.end());
        },
        accessor: quote!(#[doc = "Returns the immutable typed child placement."] #visibility const fn #symbol(&self) -> &#facade::RegisterBank<#child_type> { &self.#symbol }),
        constants,
    }
}
