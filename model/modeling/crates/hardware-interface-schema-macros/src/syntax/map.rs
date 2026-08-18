use syn::{
    Ident, LitStr, Path, Result, Token, Visibility, braced, bracketed,
    parse::{Parse, ParseStream},
};

use super::{RegisterDecl, key, keyword, usize_lit};

pub(in crate::internal) struct MapDecl {
    pub(in crate::internal) visibility: Visibility,
    pub(in crate::internal) name: Ident,
    pub(in crate::internal) size: (usize, proc_macro2::Span),
    pub(in crate::internal) fixed: Vec<RegisterDecl>,
    pub(in crate::internal) referenced_fixed: Vec<RegisterPlacementDecl>,
    pub(in crate::internal) fixed_groups: Vec<FixedRegisterGroup>,
    pub(in crate::internal) banks: Vec<RegisterGroup>,
}

pub(in crate::internal) struct FixedRegisterGroup {
    pub(in crate::internal) schema: Path,
    pub(in crate::internal) variants: Vec<Ident>,
}

pub(in crate::internal) struct RegisterGroup {
    pub(in crate::internal) name: Ident,
    pub(in crate::internal) count: (usize, proc_macro2::Span),
    pub(in crate::internal) base: (usize, proc_macro2::Span),
    pub(in crate::internal) stride: (usize, proc_macro2::Span),
    pub(in crate::internal) registers: Vec<RegisterDecl>,
    pub(in crate::internal) referenced: Vec<RegisterPlacementDecl>,
}

pub(in crate::internal) struct RegisterPlacementDecl {
    pub(in crate::internal) symbol: Ident,
    pub(in crate::internal) definition: Path,
    pub(in crate::internal) id: (u16, proc_macro2::Span),
    pub(in crate::internal) name: LitStr,
    pub(in crate::internal) offset: (usize, proc_macro2::Span),
}

enum MapMemberDecl {
    Inline(RegisterDecl),
    Reference(RegisterPlacementDecl),
}

impl Parse for MapDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let visibility = input.parse()?;
        keyword(input, "map")?;
        let name = input.parse()?;
        let body;
        braced!(body in input);
        key(&body, "size")?;
        let size = usize_lit(&body)?;
        body.parse::<Token![,]>()?;
        key(&body, "fixed")?;
        let fixed_body;
        braced!(fixed_body in body);
        let fixed_members = fixed_body.parse_terminated(MapMemberDecl::parse, Token![,])?;
        let mut fixed = Vec::new();
        let mut referenced_fixed = Vec::new();
        for member in fixed_members {
            match member {
                MapMemberDecl::Inline(register) => fixed.push(register),
                MapMemberDecl::Reference(register) => referenced_fixed.push(register),
            }
        }
        body.parse::<Token![,]>()?;
        let section: Ident = body.parse()?;
        body.parse::<Token![:]>()?;
        let fixed_groups = if section == "fixed_groups" {
            let groups_body;
            braced!(groups_body in body);
            let groups = groups_body
                .parse_terminated(FixedRegisterGroup::parse, Token![,])?
                .into_iter()
                .collect();
            body.parse::<Token![,]>()?;
            key(&body, "banks")?;
            groups
        } else if section == "banks" {
            Vec::new()
        } else {
            return Err(syn::Error::new(
                section.span(),
                "expected `fixed_groups` or `banks`",
            ));
        };
        let banks_body;
        braced!(banks_body in body);
        let banks = banks_body
            .parse_terminated(RegisterGroup::parse, Token![,])?
            .into_iter()
            .collect();
        let _ = body.parse::<Token![,]>();
        Ok(Self {
            visibility,
            name,
            size,
            fixed,
            referenced_fixed,
            fixed_groups,
            banks,
        })
    }
}

impl Parse for FixedRegisterGroup {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let schema = input.parse()?;
        input.parse::<Token![:]>()?;
        let variants_body;
        bracketed!(variants_body in input);
        let variants = variants_body
            .parse_terminated(Ident::parse, Token![,])?
            .into_iter()
            .collect();
        Ok(Self { schema, variants })
    }
}

impl Parse for RegisterGroup {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let name = input.parse()?;
        input.parse::<Token![:]>()?;
        let body;
        braced!(body in input);
        key(&body, "count")?;
        let count = usize_lit(&body)?;
        body.parse::<Token![,]>()?;
        key(&body, "base")?;
        let base = usize_lit(&body)?;
        body.parse::<Token![,]>()?;
        key(&body, "stride")?;
        let stride = usize_lit(&body)?;
        body.parse::<Token![,]>()?;
        key(&body, "members")?;
        let members;
        braced!(members in body);
        let parsed = members.parse_terminated(MapMemberDecl::parse, Token![,])?;
        let mut registers = Vec::new();
        let mut referenced = Vec::new();
        for member in parsed {
            match member {
                MapMemberDecl::Inline(register) => registers.push(register),
                MapMemberDecl::Reference(register) => referenced.push(register),
            }
        }
        Ok(Self {
            name,
            count,
            base,
            stride,
            registers,
            referenced,
        })
    }
}

impl Parse for MapMemberDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let fork = input.fork();
        let _: Ident = fork.parse()?;
        fork.parse::<Token![:]>()?;
        if fork.peek(syn::token::Brace) {
            input.parse().map(Self::Inline)
        } else {
            input.parse().map(Self::Reference)
        }
    }
}

impl Parse for RegisterPlacementDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let symbol = input.parse()?;
        input.parse::<Token![:]>()?;
        let definition = input.parse()?;
        let body;
        braced!(body in input);
        key(&body, "id")?;
        let (raw_id, span) = usize_lit(&body)?;
        let id =
            u16::try_from(raw_id).map_err(|_| syn::Error::new(span, "register ID must fit u16"))?;
        body.parse::<Token![,]>()?;
        key(&body, "name")?;
        let name = body.parse()?;
        body.parse::<Token![,]>()?;
        key(&body, "offset")?;
        let offset = usize_lit(&body)?;
        Ok(Self {
            symbol,
            definition,
            id: (id, span),
            name,
            offset,
        })
    }
}
