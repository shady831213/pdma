use syn::{
    Ident, Path, Result, Token, Visibility, braced, bracketed,
    parse::{Parse, ParseStream},
    spanned::Spanned,
};

use super::{key, keyword, usize_lit};

pub(in crate::internal) struct OrderedMapDecl {
    pub(in crate::internal) visibility: Visibility,
    pub(in crate::internal) name: Ident,
    pub(in crate::internal) alignment: (usize, proc_macro2::Span),
    pub(in crate::internal) extent: Option<(usize, proc_macro2::Span)>,
    pub(in crate::internal) placements: Vec<OrderedPlacementDecl>,
}

pub(in crate::internal) struct OrderedPlacementDecl {
    pub(in crate::internal) visibility: Visibility,
    pub(in crate::internal) symbol: Ident,
    pub(in crate::internal) definition: Path,
    pub(in crate::internal) offset: Option<(usize, proc_macro2::Span)>,
    pub(in crate::internal) count: (usize, proc_macro2::Span),
    pub(in crate::internal) stride: Option<(usize, proc_macro2::Span)>,
}

pub(in crate::internal) struct MapSyntaxProbe {
    pub(in crate::internal) ordered: bool,
}

impl Parse for MapSyntaxProbe {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let _: Visibility = input.parse()?;
        keyword(input, "map")?;
        let _: Ident = input.parse()?;
        let body;
        braced!(body in input);
        let first: Ident = body.parse()?;
        let _: proc_macro2::TokenStream = body.parse()?;
        Ok(Self {
            ordered: first == "alignment",
        })
    }
}

impl Parse for OrderedMapDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let visibility = input.parse()?;
        keyword(input, "map")?;
        let name = input.parse()?;
        let body;
        braced!(body in input);
        key(&body, "alignment")?;
        let alignment = usize_lit(&body)?;
        body.parse::<Token![,]>()?;
        let extent = if body.peek(Ident) && body.fork().parse::<Ident>()? == "extent" {
            key(&body, "extent")?;
            let extent = usize_lit(&body)?;
            body.parse::<Token![,]>()?;
            Some(extent)
        } else {
            None
        };
        key(&body, "placements")?;
        let placements_body;
        bracketed!(placements_body in body);
        let placements = placements_body
            .parse_terminated(OrderedPlacementDecl::parse, Token![,])?
            .into_iter()
            .collect();
        let _ = body.parse::<Token![,]>();
        Ok(Self {
            visibility,
            name,
            alignment,
            extent,
            placements,
        })
    }
}

impl Parse for OrderedPlacementDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let visibility = input.parse()?;
        let symbol = input.parse()?;
        input.parse::<Token![:]>()?;
        let definition: Path = input.parse()?;
        let definition_span = definition.span();
        let mut offset = None;
        let mut count = None;
        let mut stride = None;
        if input.peek(syn::token::Brace) {
            let body;
            braced!(body in input);
            while !body.is_empty() {
                let option: Ident = body.parse()?;
                body.parse::<Token![:]>()?;
                let value = usize_lit(&body)?;
                match option.to_string().as_str() {
                    "offset" if offset.is_none() => offset = Some(value),
                    "count" if count.is_none() => count = Some(value),
                    "stride" if stride.is_none() => stride = Some(value),
                    "offset" | "count" | "stride" => {
                        return Err(syn::Error::new(option.span(), "duplicate placement option"));
                    }
                    _ => return Err(syn::Error::new(option.span(), "unknown placement option")),
                }
                if !body.is_empty() {
                    body.parse::<Token![,]>()?;
                }
            }
        }
        Ok(Self {
            visibility,
            symbol,
            definition,
            offset,
            count: count.unwrap_or((1, definition_span)),
            stride,
        })
    }
}
