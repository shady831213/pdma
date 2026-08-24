use syn::{Ident, Result, Token, braced, parenthesized, parse::Parse, parse::ParseStream};

use super::ScalarType;
use crate::internal::syntax::{key, keyword, usize_lit};

pub(in crate::internal) struct BitsDecl {
    pub(in crate::internal) storage: ScalarType,
    pub(in crate::internal) fields: Vec<BitFieldDecl>,
}

pub(in crate::internal) struct BitFieldDecl {
    pub(in crate::internal) name: Ident,
    pub(in crate::internal) start: (usize, proc_macro2::Span),
    pub(in crate::internal) end: (usize, proc_macro2::Span),
}

impl Parse for BitsDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        keyword(input, "bits")?;
        let storage_body;
        parenthesized!(storage_body in input);
        let storage = ScalarType::parse(&storage_body)?;
        let body;
        braced!(body in input);
        key(&body, "fields")?;
        let fields_body;
        braced!(fields_body in body);
        let fields = fields_body
            .parse_terminated(BitFieldDecl::parse, Token![,])?
            .into_iter()
            .collect();
        let _ = body.parse::<Token![,]>();
        Ok(Self { storage, fields })
    }
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
