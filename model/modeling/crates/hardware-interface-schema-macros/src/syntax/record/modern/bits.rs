use syn::{Path, Result, Token, braced, parenthesized, parse::Parse, parse::ParseStream};

use super::ScalarType;
use crate::internal::syntax::{BitFieldDecl, key, keyword};

pub(in crate::internal) struct BitsDecl {
    pub(in crate::internal) storage: ScalarType,
    pub(in crate::internal) fields: Vec<BitFieldDecl>,
}

pub(in crate::internal) enum BitsTypeDecl {
    Inline(BitsDecl),
    Reference(Path),
}

impl Parse for BitsTypeDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        keyword(input, "bits")?;
        let storage_body;
        parenthesized!(storage_body in input);
        let probe = storage_body.fork();
        if let Ok(storage) = ScalarType::parse(&probe)
            && probe.is_empty()
        {
            let _ = ScalarType::parse(&storage_body)?;
            return parse_inline(input, storage).map(Self::Inline);
        }
        let path = storage_body.parse()?;
        if storage_body.is_empty() {
            Ok(Self::Reference(path))
        } else {
            Err(storage_body.error("unexpected bits schema path token"))
        }
    }
}

fn parse_inline(input: ParseStream<'_>, storage: ScalarType) -> Result<BitsDecl> {
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
    Ok(BitsDecl { storage, fields })
}
