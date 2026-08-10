use syn::{
    Ident, Path, Result, Token, Visibility, braced, bracketed,
    parse::{Parse, ParseStream},
};

use super::{RegisterDecl, key, keyword, usize_lit};

pub(crate) struct MapDecl {
    pub(crate) visibility: Visibility,
    pub(crate) name: Ident,
    pub(crate) size: (usize, proc_macro2::Span),
    pub(crate) fixed: Vec<RegisterDecl>,
    pub(crate) fixed_groups: Vec<FixedRegisterGroup>,
    pub(crate) banks: Vec<RegisterGroup>,
}

pub(crate) struct FixedRegisterGroup {
    pub(crate) schema: Path,
    pub(crate) variants: Vec<Ident>,
}

pub(crate) struct RegisterGroup {
    pub(crate) name: Ident,
    pub(crate) count: (usize, proc_macro2::Span),
    pub(crate) base: (usize, proc_macro2::Span),
    pub(crate) stride: (usize, proc_macro2::Span),
    pub(crate) registers: Vec<RegisterDecl>,
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
        let fixed = fixed_body
            .parse_terminated(RegisterDecl::parse, Token![,])?
            .into_iter()
            .collect();
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
        Ok(Self {
            visibility,
            name,
            size,
            fixed,
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
        let registers = members
            .parse_terminated(RegisterDecl::parse, Token![,])?
            .into_iter()
            .collect();
        Ok(Self {
            name,
            count,
            base,
            stride,
            registers,
        })
    }
}
