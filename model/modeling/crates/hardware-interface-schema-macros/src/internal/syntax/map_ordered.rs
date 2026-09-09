use syn::{
    Ident, Path, Result, Token, Visibility, braced, bracketed,
    parse::{Parse, ParseStream},
    spanned::Spanned,
};

use super::{Documentation, key, keyword, usize_lit};

pub(in crate::internal) struct OrderedMapDecl {
    pub(in crate::internal) documentation: Documentation,
    pub(in crate::internal) visibility: Visibility,
    pub(in crate::internal) name: Ident,
    pub(in crate::internal) alignment: (usize, proc_macro2::Span),
    pub(in crate::internal) extent: Option<(usize, proc_macro2::Span)>,
    pub(in crate::internal) placements: Vec<OrderedPlacementDecl>,
    pub(in crate::internal) access: Option<Vec<AccessPathDecl>>,
}

pub(in crate::internal) struct AccessPathDecl {
    pub(in crate::internal) groups: Vec<Ident>,
    pub(in crate::internal) leaf: Ident,
    pub(in crate::internal) target: Ident,
}

pub(in crate::internal) struct OrderedPlacementDecl {
    pub(in crate::internal) documentation: Documentation,
    pub(in crate::internal) visibility: Visibility,
    pub(in crate::internal) symbol: Ident,
    pub(in crate::internal) definition: Path,
    pub(in crate::internal) offset: Option<(usize, proc_macro2::Span)>,
    pub(in crate::internal) count: (usize, proc_macro2::Span),
    pub(in crate::internal) stride: Option<(usize, proc_macro2::Span)>,
}

impl Parse for OrderedMapDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let documentation = Documentation::parse(input)?;
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
        if body.peek(Token![,]) {
            body.parse::<Token![,]>()?;
        }
        let access = if body.is_empty() {
            None
        } else {
            key(&body, "access")?;
            let access_body;
            bracketed!(access_body in body);
            let entries = access_body
                .parse_terminated(AccessPathDecl::parse, Token![,])?
                .into_iter()
                .collect();
            let _ = body.parse::<Token![,]>();
            Some(entries)
        };
        Ok(Self {
            documentation,
            visibility,
            name,
            alignment,
            extent,
            placements,
            access,
        })
    }
}

impl Parse for AccessPathDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let first = input.parse()?;
        let mut segments = vec![first];
        while input.peek(Token![.]) {
            input.parse::<Token![.]>()?;
            segments.push(input.parse()?);
        }
        input.parse::<Token![=>]>()?;
        let leaf = segments
            .pop()
            .ok_or_else(|| input.error("access path must contain a leaf name"))?;
        Ok(Self {
            groups: segments,
            leaf,
            target: input.parse()?,
        })
    }
}

impl Parse for OrderedPlacementDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let documentation = Documentation::parse(input)?;
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
            documentation,
            visibility,
            symbol,
            definition,
            offset,
            count: count.unwrap_or((1, definition_span)),
            stride,
        })
    }
}
