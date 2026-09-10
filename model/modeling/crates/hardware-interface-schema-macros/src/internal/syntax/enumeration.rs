use syn::{Ident, Result, Token, braced, parenthesized, parse::Parse, parse::ParseStream};

use super::{Documentation, ScalarType};
use crate::internal::syntax::u64_lit;

pub(in crate::internal) struct EnumDecl {
    pub(in crate::internal) storage: ScalarType,
    pub(in crate::internal) bit_width: usize,
    pub(in crate::internal) variants: Vec<EnumVariantDecl>,
}

pub(in crate::internal) struct EnumVariantDecl {
    pub(in crate::internal) documentation: Documentation,
    pub(in crate::internal) name: Ident,
    pub(in crate::internal) value: Option<(u64, proc_macro2::Span)>,
}

impl Parse for EnumDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        input.parse::<Token![enum]>()?;
        let storage_body;
        parenthesized!(storage_body in input);
        let storage = ScalarType::parse(&storage_body)?;
        Self::body(input, storage, storage.width() * 8)
    }
}

impl EnumDecl {
    pub(in crate::internal) fn bit_field(input: ParseStream<'_>, width: usize) -> Result<Self> {
        input.parse::<Token![enum]>()?;
        let storage = match width {
            0..=8 => ScalarType::U8,
            9..=16 => ScalarType::U16,
            17..=32 => ScalarType::U32,
            _ => ScalarType::U64,
        };
        Self::body(input, storage, width)
    }

    fn body(input: ParseStream<'_>, storage: ScalarType, bit_width: usize) -> Result<Self> {
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
        if !body.is_empty() {
            return Err(body.error("unexpected enum clause"));
        }
        Ok(Self {
            storage,
            bit_width,
            variants,
        })
    }
}

impl Parse for EnumVariantDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let documentation = Documentation::parse(input)?;
        let name = input.parse()?;
        let value = if input.peek(Token![=]) {
            input.parse::<Token![=]>()?;
            Some(u64_lit(input)?)
        } else {
            None
        };
        Ok(Self {
            documentation,
            name,
            value,
        })
    }
}
