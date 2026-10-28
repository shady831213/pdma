use syn::{
    Ident, Path, Result, Token, Visibility, braced, bracketed, parenthesized,
    parse::{Parse, ParseStream},
    spanned::Spanned,
};

use super::{Documentation, key, keyword, usize_lit};

pub(in crate::internal) struct OrderedMapDecl {
    pub(in crate::internal) documentation: Documentation,
    pub(in crate::internal) visibility: Visibility,
    pub(in crate::internal) name: Ident,
    pub(in crate::internal) parameters: Vec<Ident>,
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
    pub(in crate::internal) bank: bool,
    pub(in crate::internal) count_parameter: Option<Ident>,
    pub(in crate::internal) arguments: Option<Vec<syn::Expr>>,
}

impl Parse for OrderedMapDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let documentation = Documentation::parse(input)?;
        let visibility = input.parse()?;
        keyword(input, "map")?;
        let name = input.parse()?;
        let mut parameters = Vec::new();
        if input.peek(syn::token::Paren) {
            let args;
            parenthesized!(args in input);
            while !args.is_empty() {
                parameters.push(args.parse()?);
                args.parse::<Token![:]>()?;
                keyword(&args, "usize")?;
                if !args.is_empty() {
                    args.parse::<Token![,]>()?;
                }
            }
        }
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
            parameters,
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
        let arguments = if input.peek(syn::token::Paren) {
            let args;
            parenthesized!(args in input);
            Some(
                args.parse_terminated(argument, Token![,])?
                    .into_iter()
                    .collect(),
            )
        } else {
            None
        };
        let bank = input.peek(syn::token::Bracket);
        let mut count_parameter = None;
        let count = if bank {
            let body;
            bracketed!(body in input);
            let value = if body.peek(Ident) {
                count_parameter = Some(body.parse()?);
                (1, definition_span)
            } else {
                usize_lit(&body)?
            };
            if !body.is_empty() {
                return Err(body.error("expected one literal or map parameter"));
            }
            value
        } else {
            (1, definition_span)
        };
        let mut offset = None;
        if input.peek(syn::token::Brace) {
            let body;
            braced!(body in input);
            while !body.is_empty() {
                let option: Ident = body.parse()?;
                body.parse::<Token![:]>()?;
                let value = usize_lit(&body)?;
                match option.to_string().as_str() {
                    "offset" if offset.is_none() => offset = Some(value),
                    "stride" => {
                        return Err(syn::Error::new(
                            option.span(),
                            "bank stride is inferred from the child layout; declare padding with its extent",
                        ));
                    }
                    "offset" => {
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
            count,
            bank,
            count_parameter,
            arguments,
        })
    }
}

fn argument(input: ParseStream<'_>) -> Result<syn::Expr> {
    if input.peek(Ident) {
        let name: Ident = input.parse()?;
        Ok(syn::parse_quote!(#name))
    } else {
        let literal: syn::LitInt = input.parse()?;
        literal.base10_parse::<usize>()?;
        Ok(syn::parse_quote!(#literal))
    }
}
