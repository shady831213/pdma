mod map;
mod record;
mod register;

pub(crate) use map::MapDecl;
pub(crate) use record::{FieldShape, RecordDecl, ScalarType};
pub(crate) use register::{AccessValue, RegisterDecl, RegistersDecl};

use syn::{Ident, LitInt, Result, Token, parse::ParseStream};

pub(crate) fn keyword(input: ParseStream<'_>, expected: &str) -> Result<Ident> {
    let ident: Ident = input.parse()?;
    if ident == expected {
        Ok(ident)
    } else {
        Err(syn::Error::new(
            ident.span(),
            format!("expected `{expected}`"),
        ))
    }
}

pub(crate) fn key(input: ParseStream<'_>, expected: &str) -> Result<()> {
    keyword(input, expected)?;
    input.parse::<Token![:]>()?;
    Ok(())
}

pub(crate) fn usize_lit(input: ParseStream<'_>) -> Result<(usize, proc_macro2::Span)> {
    let literal: LitInt = input.parse()?;
    let value = literal.base10_parse::<usize>()?;
    Ok((value, literal.span()))
}

pub(crate) fn u64_lit(input: ParseStream<'_>) -> Result<(u64, proc_macro2::Span)> {
    let literal: LitInt = input.parse()?;
    let value = literal.base10_parse::<u64>()?;
    Ok((value, literal.span()))
}
