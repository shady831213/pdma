use syn::{
    Ident, Result, Token, Visibility, braced, parenthesized, parse::Parse, parse::ParseStream,
};

use super::{ScalarType, keyword, usize_lit};

pub(in crate::internal) struct BitsSchemaDecl {
    pub(in crate::internal) visibility: Visibility,
    pub(in crate::internal) name: Ident,
    pub(in crate::internal) storage: ScalarType,
    pub(in crate::internal) fields: Vec<BitFieldDecl>,
}

pub(in crate::internal) struct BitFieldDecl {
    pub(in crate::internal) name: Ident,
    pub(in crate::internal) start: (usize, proc_macro2::Span),
    pub(in crate::internal) end: (usize, proc_macro2::Span),
}

impl Parse for BitFieldDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let name = input.parse()?;
        input.parse::<Token![:]>()?;
        let start = usize_lit(input)?;
        input.parse::<Token![..]>()?;
        let end = usize_lit(input)?;
        Ok(Self { name, start, end })
    }
}

impl Parse for BitsSchemaDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let visibility = input.parse()?;
        keyword(input, "bits")?;
        let name = input.parse()?;
        let storage_body;
        parenthesized!(storage_body in input);
        let storage = ScalarType::parse(&storage_body)?;
        let body;
        braced!(body in input);
        let fields = body
            .parse_terminated(BitFieldDecl::parse, Token![,])?
            .into_iter()
            .collect();
        Ok(Self {
            visibility,
            name,
            storage,
            fields,
        })
    }
}
