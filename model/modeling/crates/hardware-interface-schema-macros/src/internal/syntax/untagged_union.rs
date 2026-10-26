use super::Documentation;
use syn::{
    Ident, Path, Result, Token, Visibility, braced, parenthesized,
    parse::{Parse, ParseStream},
};

pub(in crate::internal) struct UntaggedUnionDecl {
    pub documentation: Documentation,
    pub visibility: Visibility,
    pub name: Ident,
    pub cases: Vec<(Ident, Path)>,
}

impl Parse for UntaggedUnionDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let documentation = Documentation::parse(input)?;
        let visibility = input.parse()?;
        input.parse::<Token![union]>()?;
        let name = input.parse()?;
        let body;
        braced!(body in input);
        reject_tag(&body)?;
        super::key(&body, "cases")?;
        let content;
        braced!(content in body);
        let mut cases: Vec<(Ident, Path)> = Vec::new();
        while !content.is_empty() {
            let case: Ident = content.parse()?;
            if cases.iter().any(|(existing, _)| *existing == case) {
                return Err(syn::Error::new(case.span(), "duplicate union case"));
            }
            let record;
            parenthesized!(record in content);
            let path = record.parse()?;
            if !record.is_empty() {
                return Err(record.error("expected one record path"));
            }
            cases.push((case, path));
            if content.is_empty() {
                break;
            }
            content.parse::<Token![,]>()?;
        }
        if cases.is_empty() {
            return Err(content.error("union requires at least one case"));
        }
        if body.peek(Token![,]) {
            body.parse::<Token![,]>()?;
        }
        if !body.is_empty() {
            reject_tag(&body)?;
            return Err(body.error("unexpected union member"));
        }
        Ok(Self {
            documentation,
            visibility,
            name,
            cases,
        })
    }
}

fn reject_tag(input: ParseStream<'_>) -> Result<()> {
    if input
        .fork()
        .parse::<Ident>()
        .is_ok_and(|ident| ident == "tag")
    {
        return Err(input.error(
            "untagged union cannot declare `tag`; use an inline `tagged_union` field in a record",
        ));
    }
    Ok(())
}
