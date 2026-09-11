use syn::{
    Ident, Result, Token, Visibility, braced, parenthesized, parse::Parse, parse::ParseStream,
};

use super::{Documentation, EnumDecl, ScalarType, keyword, usize_lit};

pub(in crate::internal) struct BitsSchemaDecl {
    pub(in crate::internal) documentation: Documentation,
    pub(in crate::internal) visibility: Visibility,
    pub(in crate::internal) name: Ident,
    pub(in crate::internal) storage: ScalarType,
    pub(in crate::internal) fields: Vec<BitFieldDecl>,
}

pub(in crate::internal) struct BitFieldDecl {
    pub(in crate::internal) enumeration: Option<EnumDecl>,
    pub(in crate::internal) documentation: Documentation,
    pub(in crate::internal) name: Ident,
    pub(in crate::internal) start: (usize, proc_macro2::Span),
    pub(in crate::internal) end: (usize, proc_macro2::Span),
}

impl Parse for BitFieldDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let documentation = Documentation::parse(input)?;
        let name = input.parse()?;
        input.parse::<Token![:]>()?;
        let start = usize_lit(input)?;
        let inclusive = if input.peek(Token![..=]) {
            input.parse::<Token![..=]>()?;
            true
        } else {
            input.parse::<Token![..]>()?;
            false
        };
        let mut end = usize_lit(input)?;
        if inclusive {
            end.0 = end.0.checked_add(1).ok_or_else(|| {
                syn::Error::new(end.1, "inclusive bit range endpoint overflows usize")
            })?;
        }
        let enumeration = if input.peek(Token![=>]) {
            input.parse::<Token![=>]>()?;
            Some(EnumDecl::bit_field(input, end.0.saturating_sub(start.0))?)
        } else {
            None
        };
        Ok(Self {
            enumeration,
            documentation,
            name,
            start,
            end,
        })
    }
}

impl Parse for BitsSchemaDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let documentation = Documentation::parse(input)?;
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
            documentation,
            visibility,
            name,
            storage,
            fields,
        })
    }
}
