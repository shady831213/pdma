use syn::{Ident, Path, Result, Token, braced, parse::Parse, parse::ParseStream};

use super::ScalarType;
use crate::internal::syntax::key;

pub(in crate::internal) struct TaggedUnionDecl {
    pub(in crate::internal) tag: TaggedUnionTagPath,
    pub(in crate::internal) cases: Vec<TaggedUnionCaseDecl>,
}

pub(in crate::internal) struct TaggedUnionTagPath {
    pub(in crate::internal) segments: Vec<Ident>,
    pub(in crate::internal) span: proc_macro2::Span,
}

pub(in crate::internal) struct TaggedUnionCaseDecl {
    pub(in crate::internal) name: Ident,
    pub(in crate::internal) fields: Vec<TaggedUnionFieldDecl>,
    pub(in crate::internal) record: Option<Path>,
}

pub(in crate::internal) struct TaggedUnionFieldDecl {
    pub(in crate::internal) name: Ident,
    pub(in crate::internal) ty: TaggedUnionFieldType,
}

pub(in crate::internal) enum TaggedUnionFieldType {
    Scalar(ScalarType),
    Bits(Path),
}

impl Parse for TaggedUnionDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        crate::internal::syntax::keyword(input, "tagged_union")?;
        let body;
        braced!(body in input);
        key(&body, "tag")?;
        let tag = body.parse()?;
        body.parse::<Token![,]>()?;
        key(&body, "cases")?;
        let cases_body;
        braced!(cases_body in body);
        let cases = cases_body
            .parse_terminated(TaggedUnionCaseDecl::parse, Token![,])?
            .into_iter()
            .collect();
        let _ = body.parse::<Token![,]>();
        Ok(Self { tag, cases })
    }
}

impl Parse for TaggedUnionTagPath {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let self_token: Token![self] = input.parse()?;
        let mut segments = Vec::new();
        while input.peek(Token![.]) {
            input.parse::<Token![.]>()?;
            segments.push(input.parse()?);
        }
        if segments.is_empty() {
            return Err(input.error("tag path must select a field from `self`"));
        }
        Ok(Self {
            segments,
            span: self_token.span,
        })
    }
}

impl Parse for TaggedUnionCaseDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let name = input.parse()?;
        if input.peek(syn::token::Paren) {
            let body;
            syn::parenthesized!(body in input);
            let record = body.parse()?;
            if !body.is_empty() {
                return Err(body.error("expected one record type"));
            }
            return Ok(Self {
                name,
                fields: Vec::new(),
                record: Some(record),
            });
        }
        let body;
        braced!(body in input);
        let fields = body
            .parse_terminated(TaggedUnionFieldDecl::parse, Token![,])?
            .into_iter()
            .collect();
        Ok(Self {
            name,
            fields,
            record: None,
        })
    }
}

impl Parse for TaggedUnionFieldDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let name = input.parse()?;
        input.parse::<Token![:]>()?;
        let probe = input.fork();
        let ty = if let Ok(scalar) = ScalarType::parse(&probe) {
            let _ = ScalarType::parse(input)?;
            TaggedUnionFieldType::Scalar(scalar)
        } else {
            TaggedUnionFieldType::Bits(input.parse()?)
        };
        Ok(Self { name, ty })
    }
}
