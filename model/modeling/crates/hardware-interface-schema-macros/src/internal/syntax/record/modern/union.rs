use syn::{Ident, Path, Result, Token, braced, parse::Parse, parse::ParseStream};

use super::ScalarType;
use crate::internal::syntax::key;

pub(in crate::internal) struct UnionDecl {
    pub(in crate::internal) tag: ScopedPath,
    pub(in crate::internal) cases: Vec<UnionCaseDecl>,
}

pub(in crate::internal) struct ScopedPath {
    pub(in crate::internal) segments: Vec<Ident>,
    pub(in crate::internal) span: proc_macro2::Span,
}

pub(in crate::internal) struct UnionCaseDecl {
    pub(in crate::internal) name: Ident,
    pub(in crate::internal) fields: Vec<UnionFieldDecl>,
}

pub(in crate::internal) struct UnionFieldDecl {
    pub(in crate::internal) name: Ident,
    pub(in crate::internal) ty: UnionFieldType,
}

pub(in crate::internal) enum UnionFieldType {
    Scalar(ScalarType),
    Bits(Path),
}

impl Parse for UnionDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        input.parse::<Token![union]>()?;
        let body;
        braced!(body in input);
        key(&body, "tag")?;
        let tag = body.parse()?;
        body.parse::<Token![,]>()?;
        key(&body, "cases")?;
        let cases_body;
        braced!(cases_body in body);
        let cases = cases_body
            .parse_terminated(UnionCaseDecl::parse, Token![,])?
            .into_iter()
            .collect();
        let _ = body.parse::<Token![,]>();
        Ok(Self { tag, cases })
    }
}

impl Parse for ScopedPath {
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

impl Parse for UnionCaseDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let name = input.parse()?;
        let body;
        braced!(body in input);
        let fields = body
            .parse_terminated(UnionFieldDecl::parse, Token![,])?
            .into_iter()
            .collect();
        Ok(Self { name, fields })
    }
}

impl Parse for UnionFieldDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let name = input.parse()?;
        input.parse::<Token![:]>()?;
        let probe = input.fork();
        let ty = if let Ok(scalar) = ScalarType::parse(&probe) {
            let _ = ScalarType::parse(input)?;
            UnionFieldType::Scalar(scalar)
        } else {
            UnionFieldType::Bits(input.parse()?)
        };
        Ok(Self { name, ty })
    }
}
