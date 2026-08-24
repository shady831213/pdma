use syn::{
    Ident, Path, Result, Token, Visibility, braced,
    parse::{Parse, ParseStream},
};

use super::ScalarType;
use crate::internal::syntax::{key, keyword, usize_lit};

mod bits;
mod enumeration;
mod union;

pub(in crate::internal) use bits::BitsDecl;
pub(in crate::internal) use enumeration::EnumDecl;
pub(in crate::internal) use union::{ScopedPath, UnionDecl, UnionFieldDecl};

pub(in crate::internal) struct ModernRecordDecl {
    pub(in crate::internal) visibility: Visibility,
    pub(in crate::internal) name: Ident,
    pub(in crate::internal) alignment: Option<(usize, proc_macro2::Span)>,
    pub(in crate::internal) endian: Ident,
    pub(in crate::internal) fields: Vec<ModernFieldDecl>,
}

pub(in crate::internal) struct ModernFieldDecl {
    pub(in crate::internal) name: Ident,
    pub(in crate::internal) ty: ModernTypeDecl,
}

pub(in crate::internal) enum ModernTypeDecl {
    Scalar {
        scalar: ScalarType,
        constant: Option<Path>,
    },
    Enum(EnumDecl),
    Bits(BitsDecl),
    Record(Path),
    Union(UnionDecl),
}

impl Parse for ModernRecordDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let visibility = input.parse()?;
        keyword(input, "record")?;
        let name = input.parse()?;
        let body;
        braced!(body in input);
        let alignment = parse_alignment(&body)?;
        key(&body, "endian")?;
        let endian = body.parse()?;
        body.parse::<Token![,]>()?;
        key(&body, "fields")?;
        let fields_body;
        braced!(fields_body in body);
        let fields = fields_body
            .parse_terminated(ModernFieldDecl::parse, Token![,])?
            .into_iter()
            .collect();
        let _ = body.parse::<Token![,]>();
        if body.is_empty() {
            Ok(Self {
                visibility,
                name,
                alignment,
                endian,
                fields,
            })
        } else {
            Err(body.error("unexpected modern record clause"))
        }
    }
}

impl Parse for ModernFieldDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let name = input.parse()?;
        input.parse::<Token![:]>()?;
        let ty = ModernTypeDecl::parse(input)?;
        Ok(Self { name, ty })
    }
}

impl ModernTypeDecl {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        if input.peek(Token![enum]) {
            return input.parse().map(Self::Enum);
        }
        if input.peek(Token![union]) {
            return input.parse().map(Self::Union);
        }
        let probe = input.fork();
        if probe.parse::<Ident>().is_ok_and(|ident| ident == "bits") {
            return input.parse().map(Self::Bits);
        }
        let path: Path = input.parse()?;
        if let Some(scalar) = scalar_path(&path) {
            return parse_scalar_properties(input)
                .map(|constant| Self::Scalar { scalar, constant });
        }
        Ok(Self::Record(path))
    }
}

fn parse_alignment(input: ParseStream<'_>) -> Result<Option<(usize, proc_macro2::Span)>> {
    let probe = input.fork();
    let first: Ident = probe.parse()?;
    if first != "alignment" {
        return Ok(None);
    }
    key(input, "alignment")?;
    let alignment = usize_lit(input)?;
    input.parse::<Token![,]>()?;
    Ok(Some(alignment))
}

fn parse_scalar_properties(input: ParseStream<'_>) -> Result<Option<Path>> {
    if !input.peek(syn::token::Brace) {
        return Ok(None);
    }
    let props;
    braced!(props in input);
    props.parse::<Token![const]>()?;
    props.parse::<Token![:]>()?;
    props.parse().map(Some)
}

fn scalar_path(path: &Path) -> Option<ScalarType> {
    if path.leading_colon.is_some() || path.segments.len() != 1 {
        return None;
    }
    let segment = path.segments.first()?;
    match segment.ident.to_string().as_str() {
        "u8" => Some(ScalarType::U8),
        "u16" => Some(ScalarType::U16),
        "u32" => Some(ScalarType::U32),
        "u64" => Some(ScalarType::U64),
        _ => None,
    }
}
