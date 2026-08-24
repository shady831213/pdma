mod bits;
mod field;
mod map;
mod map_ordered;
mod record;
mod register;

pub(in crate::internal) use field::FieldDefinitionDecl;
pub(in crate::internal) use map::{MapDecl, RegisterPlacementDecl};
pub(in crate::internal) use map_ordered::{MapSyntaxProbe, OrderedMapDecl, OrderedPlacementDecl};
pub(in crate::internal) use record::{
    BitsDecl, EnumDecl, FieldDecl, FieldShape, ModernFieldDecl, ModernRecordDecl, ModernTypeDecl,
    RecordDecl, RecordInput, RecordPlacementDecl, ScalarType, ScopedPath, UnionDecl,
    UnionFieldDecl,
};
pub(in crate::internal) use register::{
    AccessValue, RegisterDecl, RegisterDefinitionDecl, RegisterFieldsDecl, RegisterInput,
    RegistersDecl,
};

use syn::{Ident, LitInt, Result, Token, parse::ParseStream};

pub(in crate::internal) fn keyword(input: ParseStream<'_>, expected: &str) -> Result<Ident> {
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

pub(in crate::internal) fn key(input: ParseStream<'_>, expected: &str) -> Result<()> {
    keyword(input, expected)?;
    input.parse::<Token![:]>()?;
    Ok(())
}

pub(in crate::internal) fn usize_lit(input: ParseStream<'_>) -> Result<(usize, proc_macro2::Span)> {
    let literal: LitInt = input.parse()?;
    let value = literal.base10_parse::<usize>()?;
    Ok((value, literal.span()))
}

pub(in crate::internal) fn u64_lit(input: ParseStream<'_>) -> Result<(u64, proc_macro2::Span)> {
    let literal: LitInt = input.parse()?;
    let value = literal.base10_parse::<u64>()?;
    Ok((value, literal.span()))
}
pub(in crate::internal) use bits::{BitFieldDecl, BitsSchemaDecl};
