use syn::{Ident, Result, Token, braced, parenthesized, parse::Parse, parse::ParseStream};

use super::ScalarType;
use crate::internal::syntax::u64_lit;

pub(in crate::internal) struct EnumDecl {
    pub(in crate::internal) storage: ScalarType,
    pub(in crate::internal) variants: Vec<EnumVariantDecl>,
}

pub(in crate::internal) struct EnumVariantDecl {
    pub(in crate::internal) name: Ident,
    pub(in crate::internal) value: Option<(u64, proc_macro2::Span)>,
}

impl Parse for EnumDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        input.parse::<Token![enum]>()?;
        let storage_body;
        parenthesized!(storage_body in input);
        let storage = ScalarType::parse(&storage_body)?;
        let body;
        braced!(body in input);
        crate::internal::syntax::key(&body, "variants")?;
        let variants_body;
        braced!(variants_body in body);
        let variants = variants_body
            .parse_terminated(EnumVariantDecl::parse, Token![,])?
            .into_iter()
            .collect();
        let _ = body.parse::<Token![,]>();
        Ok(Self { storage, variants })
    }
}

impl Parse for EnumVariantDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let name = input.parse()?;
        let value = if input.peek(Token![=]) {
            input.parse::<Token![=]>()?;
            Some(u64_lit(input)?)
        } else {
            None
        };
        Ok(Self { name, value })
    }
}
